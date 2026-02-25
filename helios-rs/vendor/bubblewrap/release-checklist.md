bubblewrap release checklist
============================

* Collect release notes in `NEWS`
* Update version number in `meson.build` and release date in `NEWS`
* Commit the changes
* `meson dist -C ${builddir}`
* Do any final smoke-testing, e.g. update a package, install and test it
* `git evtag sign v$VERSION`
    * Include the release notes from `NEWS` in the tag message
* `git push --atomic origin main v$VERSION`
* https://github.com/containers/bubblewrap/releases/new
    * Fill in the new version's tag in the "Tag version" box
    * Title: `$VERSION`
    * Copy the release notes into the description
    * Upload the tarball that you built with `meson dist`
    * Get the `sha256sum` of the tarball and append it to the description
    * `Publish release`
