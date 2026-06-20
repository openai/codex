import subprocess
import sys


def main() -> int:
    return subprocess.call([sys.executable, *sys.argv[1:]])


if __name__ == "__main__":
    raise SystemExit(main())
