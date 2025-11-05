#!/usr/bin/env python3
"""
Migration script: docs/plans/ → docs/blueprints/

Migrates legacy plan files to the new Blueprint Mode format.

Usage:
    python scripts/migrate_plans_to_blueprints.py [--dry-run]
"""

import argparse
import json
import os
import re
import shutil
import sys
from datetime import datetime
from pathlib import Path
from typing import Dict, List, Optional

# Add tqdm for progress visualization
try:
    from tqdm import tqdm
except ImportError:
    print("⚠️  tqdm not installed. Install with: pip install tqdm")
    # Fallback to simple print
    def tqdm(iterable, **kwargs):
        return iterable

def parse_legacy_plan(content: str) -> Optional[Dict]:
    """Parse legacy plan format to Blueprint structure"""
    lines = content.split('\n')
    
    blueprint = {
        'title': '',
        'goal': '',
        'assumptions': [],
        'approach': '',
        'mode': 'single',
        'work_items': [],
        'risks': [],
        'eval': {'tests': [], 'metrics': {}},
        'budget': {},
        'rollback': '',
        'artifacts': [],
    }
    
    # Extract title (first # heading)
    for line in lines:
        if line.startswith('# '):
            blueprint['title'] = line[2:].strip()
            break
    
    # Extract goal (under ## Goal or ## Objective)
    in_goal = False
    for line in lines:
        if re.match(r'##\s+(Goal|Objective)', line, re.IGNORECASE):
            in_goal = True
            continue
        if in_goal:
            if line.startswith('##'):
                break
            if line.strip():
                blueprint['goal'] += line.strip() + ' '
    
    blueprint['goal'] = blueprint['goal'].strip()
    
    # Extract approach
    in_approach = False
    for line in lines:
        if re.match(r'##\s+Approach', line, re.IGNORECASE):
            in_approach = True
            continue
        if in_approach:
            if line.startswith('##'):
                break
            if line.strip():
                blueprint['approach'] += line.strip() + ' '
    
    blueprint['approach'] = blueprint['approach'].strip()
    
    # Default mode
    if 'orchestrat' in content.lower():
        blueprint['mode'] = 'orchestrated'
    elif 'competition' in content.lower():
        blueprint['mode'] = 'competition'
    
    return blueprint if blueprint['title'] else None

def convert_to_blueprint_markdown(blueprint: Dict, original_filename: str) -> str:
    """Convert blueprint dict to markdown format"""
    md = []
    
    md.append(f"# {blueprint['title']}\n")
    md.append(f"**Blueprint ID**: `migrated-{original_filename}`  ")
    md.append(f"**Status**: drafting  ")
    md.append(f"**Mode**: {blueprint['mode']}  ")
    md.append(f"**Created**: {datetime.now().strftime('%Y-%m-%d %H:%M:%S UTC')}  \n")
    
    md.append("## Goal\n")
    md.append(f"{blueprint['goal']}\n")
    
    if blueprint['assumptions']:
        md.append("## Assumptions\n")
        for assumption in blueprint['assumptions']:
            md.append(f"- {assumption}")
        md.append("")
    
    if blueprint['approach']:
        md.append("## Approach\n")
        md.append(f"{blueprint['approach']}\n")
    
    md.append("## Evaluation Criteria\n")
    md.append("**Tests**: (Migrated from legacy plan)\n")
    
    md.append("## Budget\n")
    md.append("- Max tokens per step: 20000")
    md.append("- Session token cap: 100000\n")
    
    md.append("---\n")
    md.append("*Migrated from legacy plan format*\n")
    
    return '\n'.join(md)

def migrate_plans(source_dir: Path, target_dir: Path, dry_run: bool = False) -> None:
    """Migrate all plans from source to target directory"""
    
    if not source_dir.exists():
        print(f"❌ Source directory does not exist: {source_dir}")
        return
    
    # Create target directory
    if not dry_run:
        target_dir.mkdir(parents=True, exist_ok=True)
    
    # Find all .md files
    plan_files = list(source_dir.glob('*.md'))
    
    if not plan_files:
        print(f"ℹ️  No plan files found in {source_dir}")
        return
    
    print(f"🔍 Found {len(plan_files)} plan files to migrate")
    
    migrated = 0
    failed = 0
    
    for plan_file in tqdm(plan_files, desc="Migrating plans"):
        try:
            # Read legacy plan
            content = plan_file.read_text(encoding='utf-8')
            
            # Parse to blueprint structure
            blueprint = parse_legacy_plan(content)
            
            if not blueprint:
                print(f"⚠️  Skipped (no title): {plan_file.name}")
                continue
            
            # Convert to new format
            new_content = convert_to_blueprint_markdown(blueprint, plan_file.stem)
            
            # Write to target
            target_file = target_dir / plan_file.name
            
            if dry_run:
                print(f"[DRY RUN] Would migrate: {plan_file.name} → {target_file}")
            else:
                target_file.write_text(new_content, encoding='utf-8')
                print(f"✅ Migrated: {plan_file.name}")
            
            migrated += 1
            
        except Exception as e:
            print(f"❌ Failed to migrate {plan_file.name}: {e}")
            failed += 1
    
    print(f"\n📊 Migration Summary:")
    print(f"  ✅ Migrated: {migrated}")
    print(f"  ❌ Failed: {failed}")
    print(f"  📁 Total: {len(plan_files)}")
    
    if not dry_run and migrated > 0:
        print(f"\n✨ Migration completed! Blueprints saved to: {target_dir}")
        print(f"ℹ️  Original plans remain in: {source_dir}")
        print(f"💡 Review migrated blueprints and delete legacy plans when ready")

def main():
    parser = argparse.ArgumentParser(
        description='Migrate legacy plans to Blueprint Mode format'
    )
    parser.add_argument(
        '--dry-run',
        action='store_true',
        help='Show what would be migrated without making changes'
    )
    parser.add_argument(
        '--source',
        type=str,
        default='docs/plans',
        help='Source directory for legacy plans (default: docs/plans)'
    )
    parser.add_argument(
        '--target',
        type=str,
        default='docs/blueprints',
        help='Target directory for blueprints (default: docs/blueprints)'
    )
    
    args = parser.parse_args()
    
    source_dir = Path(args.source)
    target_dir = Path(args.target)
    
    print("🚀 Blueprint Migration Tool")
    print(f"📂 Source: {source_dir}")
    print(f"📂 Target: {target_dir}")
    
    if args.dry_run:
        print("🔍 DRY RUN MODE (no files will be modified)")
    
    print()
    
    migrate_plans(source_dir, target_dir, args.dry_run)
    
    print("\n✅ Migration complete!")

if __name__ == '__main__':
    main()

