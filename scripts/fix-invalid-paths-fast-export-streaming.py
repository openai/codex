#!/usr/bin/env python3
"""
Git履歴から無効なパス名と大きなファイルを削除するスクリプト（ストリーミング版）
git fast-export/fast-importを使用して履歴を書き換えます。
"""

import sys
import io
import subprocess
import logging
from pathlib import Path
from datetime import datetime
from tqdm import tqdm
import threading

# Windowsでのエンコーディング問題を回避
if sys.platform == 'win32':
    sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding='utf-8', errors='replace')
    sys.stderr = io.TextIOWrapper(sys.stderr.buffer, encoding='utf-8', errors='replace')

# ログ設定
log_file = Path(".git/fix-invalid-paths.log")
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(levelname)s - %(message)s',
    handlers=[
        logging.FileHandler(log_file, encoding='utf-8'),
        logging.StreamHandler(sys.stdout)
    ]
)
logger = logging.getLogger(__name__)

# 大きなファイルのサイズ制限（100MB）
LARGE_FILE_SIZE = 100 * 1024 * 1024

def is_invalid_path(path):
    """無効なパス名をチェック"""
    if not path:
        return True
    # Windowsで無効な文字: < > : " | ? * \ および制御文字
    invalid_chars = ['<', '>', ':', '"', '|', '?', '*', '\\']
    for char in invalid_chars:
        if char in path:
            return True
    # パスが空または空白のみ
    if not path.strip():
        return True
    # パスが`>`で終わる（codex-cli/>のような無効なパス）
    if path.endswith('>'):
        return True
    return False

def is_large_file(path):
    """大きなファイルかどうかをチェック（パス名から推測）"""
    # node_modulesや.next/cacheなどの大きなファイルを含む可能性のあるパス
    large_file_patterns = [
        'node_modules',
        '.next/cache',
        '.pack',
        '.node',
    ]
    return any(pattern in path for pattern in large_file_patterns)

def main():
    logger.info("=" * 60)
    logger.info("Git履歴から無効なパス名と大きなファイルを削除")
    logger.info("=" * 60)
    
    # バックアップブランチを作成
    backup_name = f"backup-before-filter-{datetime.now().strftime('%Y%m%d-%H%M%S')}"
    logger.info(f"バックアップブランチを作成: {backup_name}")
    result = subprocess.run(
        f"git branch {backup_name}",
        shell=True,
        capture_output=True,
        text=True,
        encoding='utf-8',
        errors='replace'
    )
    if result.returncode != 0:
        logger.warning(f"バックアップブランチの作成に失敗: {result.stderr}")
    else:
        logger.info(f"✅ バックアップブランチ作成完了: {backup_name}")
    
    # git fast-exportで履歴をエクスポート
    export_file = Path(".git/fast-export-raw")
    if not export_file.exists():
        logger.info("fast-exportで履歴をエクスポート中...")
        with open(export_file, 'wb') as f:
            process = subprocess.Popen(
                "git fast-export --all",
                shell=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                bufsize=0
            )
            
            total_size = 0
            with tqdm(desc="fast-export", unit="B", unit_scale=True, unit_divisor=1024) as pbar:
                while True:
                    chunk = process.stdout.read(8192)
                    if not chunk:
                        break
                    f.write(chunk)
                    total_size += len(chunk)
                    pbar.update(len(chunk))
            
            stderr = process.stderr.read()
            code = process.wait()
            if code != 0:
                logger.error(f"fast-exportが失敗しました: {stderr.decode('utf-8', errors='replace')}")
                return 1
        logger.info(f"✅ fast-export完了: {total_size / 1024 / 1024:.2f} MB")
    else:
        logger.info("既存のfast-export-rawファイルを使用します")
    
    # ストリーミングでフィルタリング処理
    filtered_file = Path(".git/fast-export-filtered")
    logger.info("フィルタリング処理を開始...")
    
    # 第1パス: mark番号を収集
    skipped_marks = set()
    files_removed = 0
    
    logger.info("第1パス: mark番号を収集中...")
    with open(export_file, 'rb') as f:
        buffer = b''
        in_commit = False
        current_mark = None
        current_file_path = None
        
        file_size = export_file.stat().st_size
        with tqdm(total=file_size, desc="第1パス: mark収集", unit="B", unit_scale=True, unit_divisor=1024) as pbar:
            while True:
                chunk = f.read(8192)
                if not chunk:
                    break
                buffer += chunk
                pbar.update(len(chunk))
                
                # 行を処理
                while b'\n' in buffer:
                    line_bytes, buffer = buffer.split(b'\n', 1)
                    line = line_bytes.decode('utf-8', errors='replace')
                    
                    # コミットの開始
                    if line.startswith('commit '):
                        in_commit = True
                        continue
                    
                    # コミットの終了
                    if in_commit and line.strip() == '':
                        in_commit = False
                        continue
                    
                    # ファイル操作の開始（M, D, R, Cなど）
                    if in_commit and line.startswith(('M ', 'D ', 'R ', 'C ', 'N ')):
                        parts = line.split(None, 3)
                        if len(parts) >= 3:
                            if len(parts) > 3 and (parts[2].startswith(':') or parts[2] == 'inline'):
                                current_file_path = parts[3]
                                current_mark = parts[2] if parts[2].startswith(':') else None
                            else:
                                current_file_path = parts[2]
                                current_mark = None
                            
                            if is_invalid_path(current_file_path) or is_large_file(current_file_path):
                                files_removed += 1
                                if current_mark:
                                    skipped_marks.add(current_mark)
    
    logger.info(f"第1パス完了: {files_removed:,} ファイル削除, {len(skipped_marks):,} mark番号を記録")
    
    # 第2パス: フィルタリング処理
    logger.info("第2パス: フィルタリング処理を開始...")
    with open(export_file, 'rb') as f_in, open(filtered_file, 'wb') as f_out:
        buffer = b''
        in_commit = False
        in_blob = False
        in_data = False
        data_size = 0
        data_bytes_read = 0
        current_mark = None
        skip_current_blob = False
        
        file_size = export_file.stat().st_size
        with tqdm(total=file_size, desc="第2パス: フィルタリング", unit="B", unit_scale=True, unit_divisor=1024) as pbar:
            while True:
                chunk = f_in.read(8192)
                if not chunk:
                    if buffer:
                        # 残りのバッファを処理
                        if not skip_current_blob or not in_blob:
                            f_out.write(buffer)
                        buffer = b''
                    break
                
                buffer += chunk
                pbar.update(len(chunk))
                
                # 行を処理
                while b'\n' in buffer or (in_data and data_bytes_read < data_size):
                    if in_data and data_bytes_read < data_size:
                        # データ部分を処理
                        remaining = data_size - data_bytes_read
                        if len(buffer) >= remaining:
                            # データ部分の終了
                            if not skip_current_blob:
                                f_out.write(buffer[:remaining])
                            buffer = buffer[remaining:]
                            data_bytes_read = data_size
                            in_data = False
                            in_blob = False
                            skip_current_blob = False
                            # 改行をスキップ
                            if buffer.startswith(b'\n'):
                                if not skip_current_blob:
                                    f_out.write(b'\n')
                                buffer = buffer[1:]
                        else:
                            # データ部分の途中
                            if not skip_current_blob:
                                f_out.write(buffer)
                            data_bytes_read += len(buffer)
                            buffer = b''
                        continue
                    
                    # 行の処理
                    if b'\n' not in buffer:
                        break
                    
                    line_bytes, buffer = buffer.split(b'\n', 1)
                    line = line_bytes.decode('utf-8', errors='replace')
                    
                    # コミットの開始
                    if line.startswith('commit '):
                        in_commit = True
                        in_blob = False
                        in_data = False
                        skip_current_blob = False
                        f_out.write(line_bytes + b'\n')
                        continue
                    
                    # コミットの終了
                    if in_commit and line.strip() == '':
                        in_commit = False
                        f_out.write(line_bytes + b'\n')
                        continue
                    
                    # blob行の処理
                    if line.strip() == 'blob':
                        in_blob = True
                        in_data = False
                        skip_current_blob = False
                        current_mark = None
                        # blob行は後で処理（mark行を確認してから）
                        blob_line = line_bytes + b'\n'
                        continue
                    
                    # mark行の処理
                    if in_blob and line.startswith('mark '):
                        current_mark = line.split()[1] if len(line.split()) > 1 else None
                        skip_current_blob = (current_mark and current_mark in skipped_marks)
                        if not skip_current_blob:
                            f_out.write(blob_line)
                            f_out.write(line_bytes + b'\n')
                        continue
                    
                    # data行の処理
                    if in_blob and line.startswith('data '):
                        data_size = int(line.split()[1])
                        data_bytes_read = 0
                        in_data = True
                        if not skip_current_blob:
                            f_out.write(line_bytes + b'\n')
                        continue
                    
                    # ファイル操作の開始（M, D, R, Cなど）
                    if in_commit and line.startswith(('M ', 'D ', 'R ', 'C ', 'N ')):
                        parts = line.split(None, 3)
                        if len(parts) >= 3:
                            if len(parts) > 3 and (parts[2].startswith(':') or parts[2] == 'inline'):
                                file_path = parts[3]
                                mark_num = parts[2] if parts[2].startswith(':') else None
                            else:
                                file_path = parts[2]
                                mark_num = None
                            
                            if (is_invalid_path(file_path) or is_large_file(file_path) or
                                (mark_num and mark_num in skipped_marks)):
                                # M行をスキップ
                                continue
                            else:
                                f_out.write(line_bytes + b'\n')
                                continue
                    
                    # その他の行
                    if not skip_current_blob:
                        f_out.write(line_bytes + b'\n')
        
        # 残りのバッファを書き込み
        if buffer and not skip_current_blob:
            f_out.write(buffer)
    
    filtered_size = filtered_file.stat().st_size
    logger.info(f"✅ 保存完了: {filtered_size / 1024 / 1024:.2f} MB")
    
    # git fast-importで履歴を再インポート
    logger.info("履歴を再インポート中...")
    logger.info("注意: この処理には時間がかかります（数分〜数十分）")
    
    process = subprocess.Popen(
        "git fast-import",
        shell=True,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=False,
        bufsize=0
    )
    
    stderr_chunks = []
    
    def read_stderr():
        """stderrを非同期で読み取る"""
        try:
            while True:
                chunk = process.stderr.read(1024)
                if not chunk:
                    break
                stderr_chunks.append(chunk)
        except Exception:
            pass
    
    stderr_thread = threading.Thread(target=read_stderr, daemon=True)
    stderr_thread.start()
    
    try:
        filtered_size = filtered_file.stat().st_size
        with tqdm(total=filtered_size, desc="fast-import", unit="B", unit_scale=True, unit_divisor=1024) as pbar:
            with open(filtered_file, 'rb') as f:
                while True:
                    chunk = f.read(8192)
                    if not chunk:
                        break
                    
                    if process.poll() is not None:
                        stderr = b''.join(stderr_chunks) + (process.stderr.read() if process.stderr else b"")
                        code = process.returncode
                        logger.error(f"fast-importが予期せず終了しました (終了コード: {code})")
                        logger.error(f"エラーメッセージ: {stderr.decode('utf-8', errors='replace')}")
                        return 1
                    
                    try:
                        process.stdin.write(chunk)
                        process.stdin.flush()
                        pbar.update(len(chunk))
                    except BrokenPipeError:
                        stderr = b''.join(stderr_chunks) + (process.stderr.read() if process.stderr else b"")
                        code = process.returncode
                        logger.error(f"fast-importへのパイプが閉じられました (終了コード: {code})")
                        logger.error(f"エラーメッセージ: {stderr.decode('utf-8', errors='replace')}")
                        return 1
                    except Exception as e:
                        logger.error(f"fast-importへの書き込み中にエラーが発生しました: {e}")
                        return 1
        
        process.stdin.close()
        
        stdout = process.stdout.read() if process.stdout else b""
        stderr_thread.join(timeout=5)
        stderr = b''.join(stderr_chunks) + (process.stderr.read() if process.stderr else b"")
        code = process.wait()
        
    except Exception as e:
        logger.error(f"fast-import実行中にエラーが発生しました: {e}")
        process.kill()
        return 1
    
    if code != 0:
        stderr_text = stderr.decode('utf-8', errors='replace') if isinstance(stderr, bytes) else stderr
        logger.error(f"fast-importが失敗しました: {stderr_text}")
        logger.error("バックアップブランチから復元してください:")
        logger.error(f"  git reset --hard {backup_name}")
        return 1
    
    logger.info("✅ fast-import完了")
    
    # 一時ファイルを削除
    logger.info("一時ファイルを削除中...")
    export_file.unlink(missing_ok=True)
    filtered_file.unlink(missing_ok=True)
    logger.info("✅ 一時ファイル削除完了")
    
    logger.info("=" * 60)
    logger.info("✅ 処理完了")
    logger.info("=" * 60)
    logger.info(f"バックアップブランチ: {backup_name}")
    logger.info("次のコマンドでリモートにプッシュできます:")
    logger.info("  git push --force origin main")
    
    return 0

if __name__ == "__main__":
    sys.exit(main())

