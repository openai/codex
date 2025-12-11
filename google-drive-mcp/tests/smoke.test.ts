describe("google-drive-mcp", () => {
  it("loads the module without running main", () => {
    // eslint-disable-next-line @typescript-eslint/no-var-requires, global-require
    expect(() => require("../src/index")).not.toThrow();
  });
});

