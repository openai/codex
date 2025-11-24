# Derivative

This crate provides a set of alternative `#[derive]` attributes for Rust.

## Examples

*derivative* uses attributes to make it possible to derive more implementations
than the built-in `derive(Trait)`. Here are a few examples of stuffs you cannot
just `derive`.

You can derive `Default` on enumerations:

<table>
<tr>
<th>

With *derivative*
</th>
<th>
    
[Original][default-value-source]
</th>
</tr>
<tr>
</tr>
<tr class="readme-example">
<td>

{{#playground default-enum.rs}}
</td>
<td>

{{#playground default-enum-orig.rs}}
</td>
</tr>
</table>

You can use different default values for some fields:

<table>
<tr>
<th>

With *derivative*
</th>
<th>
    
[Original][default-value-source]
</th>
</tr>
<tr>
</tr>
<tr class="readme-example">
<td>

{{#playground default-value.rs}}
</td>
<td>

{{#playground default-value-orig.rs}}
</td>
</tr>
</table>


Want a transparent `Debug` implementation for your wrapper? We got that:

<table>
<tr>
<th>

With *derivative*
</th>
<th>
    
[Original][transparent-source]
</th>
</tr>
<tr>
</tr>
<tr class="readme-example">
<td>

{{#playground debug-transparent.rs}}
</td>
<td>

{{#playground debug-transparent-orig.rs}}
</td>
</tr>
</table>


Need to ignore a field? We got that too:

<table>
<tr>
<th>

With *derivative*
</th>
<th>
    
[Original][eq-ignore-source]
</th>
</tr>
<tr>
</tr>
<tr class="readme-example">
<td>

{{#playground eq-ignore.rs}}
</td>
<td>

{{#playground eq-ignore-orig.rs}}
</td>
</tr>
</table>


[default-value-source]: https://github.com/rust-lang-nursery/regex/blob/3cfef1e79d135a3e8a670aff53e7fabef453a3e1/src/re_builder.rs#L12-L39
[default-enum-source]: https://github.com/rust-lang/rust/blob/16eeeac783d2ede28e09f2a433c612dea309fe33/src/libcore/option.rs#L714-L718
[transparent-source]: https://github.com/rust-lang/rust/blob/5457c35ece57bbc4a65baff239a02d6abb81c8a2/src/libcore/num/mod.rs#L46-L54
[eq-ignore-source]: https://github.com/steveklabnik/semver/blob/baa0fbb57c80a7fb344fbeedac24a28439ddf5b5/src/version.rs#L196-L205
