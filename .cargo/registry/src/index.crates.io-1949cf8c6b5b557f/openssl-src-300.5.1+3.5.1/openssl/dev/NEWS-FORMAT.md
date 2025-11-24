Editorial conventions for NEWS.md
=================================

With 3.2 onwards we are seeking to make our NEWS.md file more like release
notes, structured to provide useful and pertinent information to those consuming
a release.

General editorial principles
----------------------------

- Use `*` for top-level lists and use a blank line between each list item.
  This makes the file more readable when read in raw form, which will commonly
  occur when a user examines an unpacked tarball.

- Cite RFCs with a space: `RFC 9000`

- Put URLs at the end of the file and reference them rather than giving them
  inline. This eases maintenance if a commonly used URL needs to be changed.

- The blocks within a section for a release line are ordered roughly in
  descending order of importance. Equally, list items within a block should be
  listed in descending order of significance or severity.

- Try to develop blog posts to match our wording, especially for the list of
  new features.

- Adopt uniform wording for lists where appropriate but do not insist on
  sticking to them where these wordings are unworkable or suboptimal.

- Everything here is a recommendation, not a requirement.

- Omit blocks which have no items to list.

Structure
---------

Each release line has a section, which is broken down into the initial and patch
releases within that release line. The most recent releases come first.

The structure is as follows:

```text
OpenSSL x.y
-----------

<entry for patch releases of OpenSSL x.y...>
<entry for patch releases of OpenSSL x.y...>
<entry for initial (feature) release of OpenSSL x.y>
```

### Structure of a release entry

For each release in a release line, the recommended structure is as follows:

```text
### Major changes between OpenSSL {PREV_VERSION} and OpenSSL {VERSION} [1 Jan 2023]

<opener paragraph>

<one or more blocks listed below as applicable, in order shown below>

<trailing advice>
```

#### Opener paragraph

For a feature release, the following opener paragraph is suggested:

```text
OpenSSL x.y.0 is a feature release adding significant new functionality to
OpenSSL.
```

For a patch release with no CVEs fixed, the following opener paragraph is
suggested:

```text
OpenSSL x.y.z is a patch release.
```

For a patch release which fixes one or more CVEs, the following opener paragraph
is suggested, to be adjusted as appropriate:

```text
OpenSSL x.y.z is a security patch release. The most severe CVE fixed in this
release is Medium.
```

#### Listing potentially incompatible changes

If there are any potentially significant or incompatible changes, the following
block should be added:

```text
This release incorporates the following potentially significant or incompatible
changes:

    * The ... has been changed so that xxx.

    * The ... has been changed so that yyy.

```

Bullet items in this block should be complete sentences with trailing full stops
giving a brief summary. They may optionally be followed by full paragraphs
giving further information if needed.

#### Listing feature additions

If there are any feature additions, the following block should be added:

```text
This release adds the following new features:

    * Support for ... (RFC 1234)

    * Support for ... (RFC 2345)

      This is an elaborating paragraph.

    * Multiple new features and improvements to ...

```

Bullet items in this block should be summary lines without a trailing full stop
giving a brief summary, optionally followed by references to applicable
standards in parentheses. They may optionally be followed by full paragraphs
giving further information if needed. The summary line should not start with a
verb as the opener line for this block provides the verb.

For consistency, use the wording `Support for ...` as the summary line if
possible and practical.

List features in descending order of significance (approximately).

#### Listing known issues

Known issues can be called out as follows:

```text
The following known issues are present in this release and will be rectified in
a future release:

    * xxx (#12345)

```

The editorial conventions for this block are similar to those for feature
additions, except that an issue number is listed rather than a reference to a
standard.

#### Listing documentation enhancements

Significant documentation enhancements can be called out as follows:

```text
This release incorporates the following documentation enhancements:

    * Added xyz

      This is an elaborating paragraph, which might for example
      provide a link to where this documentation can be viewed.

    * Clarified xyz

```

The editorial conventions for this block are similar to those for feature
additions, except that the verb is part of the summary line. The other rules are
the same.

For consistency, use the wording `Added ...` or `Clarified ...` as the summary
line if possible.

#### Listing bug fixes and mitigations

Significant bug fixes or mitigations can be called out as follows:

```text
This release incorporates the following bug fixes and mitigations:

    * Mitigated <description of mitigation> (CVE ID as link and any other
      relevant links)

    * Fixed <description of fix> (optional reference link or #issue number as
      appropriate)
```

The words “bug fixes” or “mitigations” in the leader line should be deleted as
appropriate if inapplicable to a release.

Fixes for issues with an issue number in the main repository should be given as
`#1234`. Any other issue (for example, a project issue) should be given as a
link, as most users will not know where to find such issues.

List CVE mitigations first in descending order of severity, followed by bugs in
(very rough) descending order of severity.

#### Trailing advice

The following trailer is recommended:

```text
A more detailed list of changes in this release can be found in the
[CHANGES.md] file.

As always, bug reports and issues relating to OpenSSL can be [filed on our issue
tracker][issue tracker].
```
