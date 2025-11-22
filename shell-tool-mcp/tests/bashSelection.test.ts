import { selectDarwinBash, selectLinuxBash } from "../src/bashSelection";
import { DARWIN_BASH_VARIANTS, LINUX_BASH_VARIANTS } from "../src/constants";
import { OsReleaseInfo } from "../src/types";
import path from "node:path";

describe("selectLinuxBash", () => {
  const bashRoot = "/vendor/bash";

  it("prefers exact version match when id is present", () => {
    const info: OsReleaseInfo = {
      id: "ubuntu",
      idLike: ["debian"],
      versionId: "24.04.1",
    };
    const selection = selectLinuxBash(bashRoot, info);
    expect(selection.variant).toBe("ubuntu-24.04");
    expect(selection.path).toBe(path.join(bashRoot, "ubuntu-24.04", "bash"));
  });

  it("falls back to first supported variant when no matches", () => {
    const info: OsReleaseInfo = { id: "unknown", idLike: [], versionId: "1.0" };
    const selection = selectLinuxBash(bashRoot, info);
    expect(selection.variant).toBe(LINUX_BASH_VARIANTS[0].name);
  });
});

describe("selectDarwinBash", () => {
  const bashRoot = "/vendor/bash";

  it("selects compatible darwin version", () => {
    const darwinRelease = "24.0.0";
    const selection = selectDarwinBash(bashRoot, darwinRelease);
    expect(selection.variant).toBe("macos-15");
  });

  it("falls back to first darwin variant when release too old", () => {
    const darwinRelease = "20.0.0";
    const selection = selectDarwinBash(bashRoot, darwinRelease);
    expect(selection.variant).toBe(DARWIN_BASH_VARIANTS[0].name);
  });
});
