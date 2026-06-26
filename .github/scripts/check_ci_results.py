#!/usr/bin/env python3

import json
import os


def main() -> None:
    needs = json.loads(os.environ["NEEDS"])
    failures = sorted(
        (name, dependency["result"])
        for name, dependency in needs.items()
        if dependency["result"] != "success"
    )

    if failures:
        print("CI dependencies did not succeed:")
        for name, result in failures:
            print(f"{name}: {result}")
        raise SystemExit(1)

    print("All CI dependencies succeeded.")


if __name__ == "__main__":
    main()
