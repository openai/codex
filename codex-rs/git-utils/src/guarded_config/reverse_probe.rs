use std::io;

use super::GuardedGitConfig;

#[derive(Clone, Copy)]
enum IntentToAddView {
    Invisible,
    Visible,
}

impl<'git> GuardedGitConfig<'git> {
    pub(crate) fn reverse_index_stage_output(
        &self,
        paths: &[String],
    ) -> io::Result<std::process::Output> {
        let mut command = self.command_with_attached_overlays()?;
        command.args([
            "--literal-pathspecs",
            "ls-files",
            "-v",
            "--stage",
            "-z",
            "--",
        ]);
        command.args(paths);
        self.sources.git.output(command)
    }

    pub(crate) fn reverse_cached_name_status_invisible_output(
        &self,
        paths: &[String],
    ) -> io::Result<std::process::Output> {
        self.reverse_cached_name_status_output(paths, IntentToAddView::Invisible)
    }

    pub(crate) fn reverse_cached_name_status_visible_output(
        &self,
        paths: &[String],
    ) -> io::Result<std::process::Output> {
        self.reverse_cached_name_status_output(paths, IntentToAddView::Visible)
    }

    fn reverse_cached_name_status_output(
        &self,
        paths: &[String],
        view: IntentToAddView,
    ) -> io::Result<std::process::Output> {
        let intent_to_add = match view {
            IntentToAddView::Invisible => "--ita-invisible-in-index",
            IntentToAddView::Visible => "--ita-visible-in-index",
        };
        let mut command = self.command_with_attached_overlays()?;
        command.args([
            "--literal-pathspecs",
            "diff",
            "--cached",
            "--name-status",
            "-z",
            "--no-renames",
            intent_to_add,
            "--no-ext-diff",
            "--no-textconv",
            "--ignore-submodules=none",
            "--",
        ]);
        command.args(paths);
        self.sources.git.output(command)
    }

    pub(crate) fn reverse_worktree_changed_paths_output(
        &self,
        paths: &[String],
    ) -> io::Result<std::process::Output> {
        let mut command = self.command_with_attached_overlays()?;
        #[cfg(unix)]
        command.args(["-c", "core.filemode=true"]);
        command.args([
            "--literal-pathspecs",
            "diff-files",
            "--name-only",
            "-z",
            "--no-renames",
            "--no-ext-diff",
            "--no-textconv",
            "--ignore-submodules=none",
            "--",
        ]);
        command.args(paths);
        self.sources.git.output(command)
    }
}
