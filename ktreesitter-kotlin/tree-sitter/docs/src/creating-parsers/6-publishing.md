# Publishing your grammar

Once you feel that your parser is in a stable working state for consumers to use, you can publish it to various registries.
It's strongly recommended to publish grammars to GitHub, [crates.io][crates.io] (Rust), [npm][npm] (JavaScript), and [PyPI][pypi]
(Python) to make it easier for others to find and use your grammar.

If your grammar is hosted on GitHub, you can make use of our [reusable workflows][workflows] to handle the publishing process
for you. This action will automatically handle regenerating and publishing your grammar in CI, so long as you have the required
tokens setup for the various registries. For an example of this workflow in action, see the [Python grammar's GitHub][python-gh]

## From start to finish

To release a new grammar (or publish your first version), these are the steps you should follow:

1. Bump your version to the desired version with `tree-sitter version`. For example, if you're releasing version `1.0.0`
of your grammar, you'd run `tree-sitter version 1.0.0`.
2. Commit the changes with `git commit -am "Release 1.0.0" (or however you like)` (ensure that your working directory is
clean).
3. Tag the commit with `git tag -- v1.0.0`.
4. Push the commit and tag with `git push --tags origin main` (assuming you're on the `main` branch, and `origin` is your
remote).
5. (optional) If you've set up the GitHub workflows for your grammar, the release will be automatically published to GitHub,
crates.io, npm, and PyPI.

### Adhering to Semantic Versioning

When releasing new versions of your grammar, it's important to adhere to [Semantic Versioning][semver]. This ensures that
consumers can predictably update their dependencies and that their existing tree-sitter integrations (queries, tree traversal
code, node type checks) will continue to work as expected when upgrading.

1. Increment the major version when you make incompatible changes to the grammar's node types or structure
2. Increment the minor version when you add new node types or patterns while maintaining backward compatibility
3. Increment the patch version when you fix bugs without changing the grammar's structure

For grammars in version 0.y.z (zero version), the usual semantic versioning rules are technically relaxed. However, if your
grammar already has users, it's recommended to treat version changes more conservatively:

- Treat patch version (`z`) changes as if they were minor version changes
- Treat minor version (`y`) changes as if they were major version changes

This helps maintain stability for existing users during the pre-1.0 phase. By following these versioning guidelines, you
ensure that downstream users can safely upgrade without their existing queries breaking.

[crates.io]: https://crates.io
[npm]: https://www.npmjs.com
[pypi]: https://pypi.org
[python-gh]: https://github.com/tree-sitter/tree-sitter-python/blob/master/.github/workflows/publish.yml
[semver]: https://semver.org/
[workflows]: https://github.com/tree-sitter/workflows
