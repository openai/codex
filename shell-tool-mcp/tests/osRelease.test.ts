import { parseOsRelease } from "../src/osRelease";

describe("parseOsRelease", () => {
  it("parses basic fields", () => {
    const contents = `ID="ubuntu"
ID_LIKE="debian"
VERSION_ID=24.04
OTHER=ignored`;

    const info = parseOsRelease(contents);
    expect(info).toEqual({
      id: "ubuntu",
      idLike: ["debian"],
      versionId: "24.04",
    });
  });

  it("handles missing fields", () => {
    const contents = "SOMETHING=else";
    const info = parseOsRelease(contents);
    expect(info).toEqual({ id: "", idLike: [], versionId: "" });
  });

  it("normalizes id_like entries", () => {
    const contents = `ID="rhel"
ID_LIKE="CentOS   Rocky"`;
    const info = parseOsRelease(contents);
    expect(info.idLike).toEqual(["centos", "rocky"]);
  });
});
