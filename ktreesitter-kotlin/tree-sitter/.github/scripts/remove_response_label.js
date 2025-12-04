module.exports = async ({ github, context }) => {
  const commenter = context.actor;
  const issue = await github.rest.issues.get({
    owner: context.repo.owner,
    repo: context.repo.repo,
    issue_number: context.issue.number,
  });
  const author = issue.data.user.login;
  const labels = issue.data.labels.map((e) => e.name);

  if (author === commenter && labels.includes("more-information-needed")) {
    github.rest.issues.removeLabel({
      owner: context.repo.owner,
      repo: context.repo.repo,
      issue_number: context.issue.number,
      name: "more-information-needed",
    });
  }
};
