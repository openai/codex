module.exports = async ({ github, context }) => {
  const requestedReviewers = await github.rest.pulls.listRequestedReviewers({
    owner: context.repo.owner,
    repo: context.repo.repo,
    pull_number: context.issue.number,
  });

  const reviewers = requestedReviewers.data.users.map((e) => e.login);

  github.rest.pulls.removeRequestedReviewers({
    owner: context.repo.owner,
    repo: context.repo.repo,
    pull_number: context.issue.number,
    reviewers: reviewers,
  });
};
