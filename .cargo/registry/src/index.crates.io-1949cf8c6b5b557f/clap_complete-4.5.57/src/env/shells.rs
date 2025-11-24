use std::ffi::OsString;
use std::str::FromStr;

use super::EnvCompleter;

/// Bash completion adapter
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Bash;

impl EnvCompleter for Bash {
    fn name(&self) -> &'static str {
        "bash"
    }
    fn is(&self, name: &str) -> bool {
        name == "bash"
    }
    fn write_registration(
        &self,
        var: &str,
        name: &str,
        bin: &str,
        completer: &str,
        buf: &mut dyn std::io::Write,
    ) -> Result<(), std::io::Error> {
        let escaped_name = name.replace('-', "_");

        let completer =
            shlex::try_quote(completer).unwrap_or(std::borrow::Cow::Borrowed(completer));

        let script = r#"
_clap_complete_NAME() {
    local IFS=$'\013'
    local _CLAP_COMPLETE_INDEX=${COMP_CWORD}
    local _CLAP_COMPLETE_COMP_TYPE=${COMP_TYPE}
    if compopt +o nospace 2> /dev/null; then
        local _CLAP_COMPLETE_SPACE=false
    else
        local _CLAP_COMPLETE_SPACE=true
    fi
    local words=("${COMP_WORDS[@]}")
    if [[ "${BASH_VERSINFO[0]}" -ge 4 ]]; then
        words[COMP_CWORD]="$2"
    fi
    COMPREPLY=( $( \
        _CLAP_IFS="$IFS" \
        _CLAP_COMPLETE_INDEX="$_CLAP_COMPLETE_INDEX" \
        _CLAP_COMPLETE_COMP_TYPE="$_CLAP_COMPLETE_COMP_TYPE" \
        _CLAP_COMPLETE_SPACE="$_CLAP_COMPLETE_SPACE" \
        VAR="bash" \
        "COMPLETER" -- "${words[@]}" \
    ) )
    if [[ $? != 0 ]]; then
        unset COMPREPLY
    elif [[ $_CLAP_COMPLETE_SPACE == false ]] && [[ "${COMPREPLY-}" =~ [=/:]$ ]]; then
        compopt -o nospace
    fi
}
if [[ "${BASH_VERSINFO[0]}" -eq 4 && "${BASH_VERSINFO[1]}" -ge 4 || "${BASH_VERSINFO[0]}" -gt 4 ]]; then
    complete -o nospace -o bashdefault -o nosort -F _clap_complete_NAME BIN
else
    complete -o nospace -o bashdefault -F _clap_complete_NAME BIN
fi
"#
        .replace("NAME", &escaped_name)
        .replace("BIN", bin)
        .replace("COMPLETER", &completer)
        .replace("VAR", var);

        writeln!(buf, "{script}")?;
        Ok(())
    }
    fn write_complete(
        &self,
        cmd: &mut clap::Command,
        args: Vec<OsString>,
        current_dir: Option<&std::path::Path>,
        buf: &mut dyn std::io::Write,
    ) -> Result<(), std::io::Error> {
        let index: usize = std::env::var("_CLAP_COMPLETE_INDEX")
            .ok()
            .and_then(|i| i.parse().ok())
            .unwrap_or_default();
        let _comp_type: CompType = std::env::var("_CLAP_COMPLETE_COMP_TYPE")
            .ok()
            .and_then(|i| i.parse().ok())
            .unwrap_or_default();
        let _space: Option<bool> = std::env::var("_CLAP_COMPLETE_SPACE")
            .ok()
            .and_then(|i| i.parse().ok());
        let ifs: Option<String> = std::env::var("_CLAP_IFS").ok().and_then(|i| i.parse().ok());
        let completions = crate::engine::complete(cmd, args, index, current_dir)?;

        for (i, candidate) in completions.iter().enumerate() {
            if i != 0 {
                write!(buf, "{}", ifs.as_deref().unwrap_or("\n"))?;
            }
            write!(buf, "{}", candidate.get_value().to_string_lossy())?;
        }
        Ok(())
    }
}

/// Type of completion attempted that caused a completion function to be called
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
enum CompType {
    /// Normal completion
    Normal,
    /// List completions after successive tabs
    Successive,
    /// List alternatives on partial word completion
    Alternatives,
    /// List completions if the word is not unmodified
    Unmodified,
    /// Menu completion
    Menu,
}

impl FromStr for CompType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "9" => Ok(Self::Normal),
            "63" => Ok(Self::Successive),
            "33" => Ok(Self::Alternatives),
            "64" => Ok(Self::Unmodified),
            "37" => Ok(Self::Menu),
            _ => Err(format!("unsupported COMP_TYPE `{s}`")),
        }
    }
}

impl Default for CompType {
    fn default() -> Self {
        Self::Normal
    }
}

/// Elvish completion adapter
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Elvish;

impl EnvCompleter for Elvish {
    fn name(&self) -> &'static str {
        "elvish"
    }
    fn is(&self, name: &str) -> bool {
        name == "elvish"
    }
    fn write_registration(
        &self,
        var: &str,
        _name: &str,
        bin: &str,
        completer: &str,
        buf: &mut dyn std::io::Write,
    ) -> Result<(), std::io::Error> {
        let bin = shlex::try_quote(bin).unwrap_or(std::borrow::Cow::Borrowed(bin));
        let completer =
            shlex::try_quote(completer).unwrap_or(std::borrow::Cow::Borrowed(completer));

        let script = r#"
set edit:completion:arg-completer[BIN] = { |@words|
    var index = (count $words)
    set index = (- $index 1)

    put (env _CLAP_IFS="\n" _CLAP_COMPLETE_INDEX=(to-string $index) VAR="elvish" COMPLETER -- $@words) | to-lines
}
"#
        .replace("COMPLETER", &completer)
        .replace("BIN", &bin)
        .replace("VAR", var);

        writeln!(buf, "{script}")?;
        Ok(())
    }
    fn write_complete(
        &self,
        cmd: &mut clap::Command,
        args: Vec<OsString>,
        current_dir: Option<&std::path::Path>,
        buf: &mut dyn std::io::Write,
    ) -> Result<(), std::io::Error> {
        let index: usize = std::env::var("_CLAP_COMPLETE_INDEX")
            .ok()
            .and_then(|i| i.parse().ok())
            .unwrap_or_default();
        let ifs: Option<String> = std::env::var("_CLAP_IFS").ok().and_then(|i| i.parse().ok());
        let completions = crate::engine::complete(cmd, args, index, current_dir)?;

        for (i, candidate) in completions.iter().enumerate() {
            if i != 0 {
                write!(buf, "{}", ifs.as_deref().unwrap_or("\n"))?;
            }
            write!(buf, "{}", candidate.get_value().to_string_lossy())?;
        }
        Ok(())
    }
}

/// Fish completion adapter
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Fish;

impl EnvCompleter for Fish {
    fn name(&self) -> &'static str {
        "fish"
    }
    fn is(&self, name: &str) -> bool {
        name == "fish"
    }
    fn write_registration(
        &self,
        var: &str,
        _name: &str,
        bin: &str,
        completer: &str,
        buf: &mut dyn std::io::Write,
    ) -> Result<(), std::io::Error> {
        let bin = shlex::try_quote(bin).unwrap_or(std::borrow::Cow::Borrowed(bin));
        let completer =
            shlex::try_quote(completer).unwrap_or(std::borrow::Cow::Borrowed(completer));

        writeln!(
            buf,
            r#"complete --keep-order --exclusive --command {bin} --arguments "({var}=fish "'{completer}'" -- (commandline --current-process --tokenize --cut-at-cursor) (commandline --current-token))""#
        )
    }
    fn write_complete(
        &self,
        cmd: &mut clap::Command,
        args: Vec<OsString>,
        current_dir: Option<&std::path::Path>,
        buf: &mut dyn std::io::Write,
    ) -> Result<(), std::io::Error> {
        let index = args.len() - 1;
        let completions = crate::engine::complete(cmd, args, index, current_dir)?;

        for candidate in completions {
            write!(buf, "{}", candidate.get_value().to_string_lossy())?;
            if let Some(help) = candidate.get_help() {
                write!(
                    buf,
                    "\t{}",
                    help.to_string().lines().next().unwrap_or_default()
                )?;
            }
            writeln!(buf)?;
        }
        Ok(())
    }
}

/// Powershell completion adapter
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Powershell;

impl EnvCompleter for Powershell {
    fn name(&self) -> &'static str {
        "powershell"
    }
    fn is(&self, name: &str) -> bool {
        name == "powershell" || name == "powershell_ise"
    }
    fn write_registration(
        &self,
        var: &str,
        _name: &str,
        bin: &str,
        completer: &str,
        buf: &mut dyn std::io::Write,
    ) -> Result<(), std::io::Error> {
        let bin = shlex::try_quote(bin).unwrap_or(std::borrow::Cow::Borrowed(bin));
        let completer =
            shlex::try_quote(completer).unwrap_or(std::borrow::Cow::Borrowed(completer));

        // `completer` may or may not be surrounded by double quotes, enclosing
        // the expression in a here-string ensures the whole thing is
        // interpreted as the first argument to the call operator
        writeln!(
            buf,
            r#"
Register-ArgumentCompleter -Native -CommandName {bin} -ScriptBlock {{
    param($wordToComplete, $commandAst, $cursorPosition)

    $prev = $env:{var};
    $env:{var} = "powershell";
    $results = Invoke-Expression @"
& {completer} -- $commandAst
"@;
    if ($null -eq $prev) {{
        Remove-Item Env:\{var};
    }} else {{
        $env:{var} = $prev;
    }}
    $results | ForEach-Object {{
        $split = $_.Split("`t");
        $cmd = $split[0];

        if ($split.Length -eq 2) {{
            $help = $split[1];
        }}
        else {{
            $help = $split[0];
        }}

        [System.Management.Automation.CompletionResult]::new($cmd, $cmd, 'ParameterValue', $help)
    }}
}};
        "#
        )
    }

    fn write_complete(
        &self,
        cmd: &mut clap::Command,
        args: Vec<OsString>,
        current_dir: Option<&std::path::Path>,
        buf: &mut dyn std::io::Write,
    ) -> Result<(), std::io::Error> {
        let index = args.len() - 1;
        let completions = crate::engine::complete(cmd, args, index, current_dir)?;

        for candidate in completions {
            write!(buf, "{}", candidate.get_value().to_string_lossy())?;
            if let Some(help) = candidate.get_help() {
                write!(
                    buf,
                    "\t{}",
                    help.to_string().lines().next().unwrap_or_default()
                )?;
            }
            writeln!(buf)?;
        }
        Ok(())
    }
}

/// Zsh completion adapter
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Zsh;

impl EnvCompleter for Zsh {
    fn name(&self) -> &'static str {
        "zsh"
    }
    fn is(&self, name: &str) -> bool {
        name == "zsh"
    }
    fn write_registration(
        &self,
        var: &str,
        name: &str,
        bin: &str,
        completer: &str,
        buf: &mut dyn std::io::Write,
    ) -> Result<(), std::io::Error> {
        let escaped_name = name.replace('-', "_");
        let bin = shlex::try_quote(bin).unwrap_or(std::borrow::Cow::Borrowed(bin));
        let completer =
            shlex::try_quote(completer).unwrap_or(std::borrow::Cow::Borrowed(completer));

        let script = r#"#compdef BIN
function _clap_dynamic_completer_NAME() {
    local _CLAP_COMPLETE_INDEX=$(expr $CURRENT - 1)
    local _CLAP_IFS=$'\n'

    local completions=("${(@f)$( \
        _CLAP_IFS="$_CLAP_IFS" \
        _CLAP_COMPLETE_INDEX="$_CLAP_COMPLETE_INDEX" \
        VAR="zsh" \
        COMPLETER -- "${words[@]}" 2>/dev/null \
    )}")

    if [[ -n $completions ]]; then
        _describe 'values' completions
    fi
}

compdef _clap_dynamic_completer_NAME BIN"#
            .replace("NAME", &escaped_name)
            .replace("COMPLETER", &completer)
            .replace("BIN", &bin)
            .replace("VAR", var);

        writeln!(buf, "{script}")?;
        Ok(())
    }
    fn write_complete(
        &self,
        cmd: &mut clap::Command,
        args: Vec<OsString>,
        current_dir: Option<&std::path::Path>,
        buf: &mut dyn std::io::Write,
    ) -> Result<(), std::io::Error> {
        let index: usize = std::env::var("_CLAP_COMPLETE_INDEX")
            .ok()
            .and_then(|i| i.parse().ok())
            .unwrap_or_default();
        let ifs: Option<String> = std::env::var("_CLAP_IFS").ok().and_then(|i| i.parse().ok());

        // If the current word is empty, add an empty string to the args
        let mut args = args.clone();
        if args.len() == index {
            args.push("".into());
        }
        let completions = crate::engine::complete(cmd, args, index, current_dir)?;

        for (i, candidate) in completions.iter().enumerate() {
            if i != 0 {
                write!(buf, "{}", ifs.as_deref().unwrap_or("\n"))?;
            }
            write!(
                buf,
                "{}",
                Self::escape_value(&candidate.get_value().to_string_lossy())
            )?;
            if let Some(help) = candidate.get_help() {
                write!(
                    buf,
                    ":{}",
                    Self::escape_help(help.to_string().lines().next().unwrap_or_default())
                )?;
            }
        }
        Ok(())
    }
}

impl Zsh {
    /// Escape value string
    fn escape_value(string: &str) -> String {
        string.replace('\\', "\\\\").replace(':', "\\:")
    }

    /// Escape help string
    fn escape_help(string: &str) -> String {
        string.replace('\\', "\\\\")
    }
}
