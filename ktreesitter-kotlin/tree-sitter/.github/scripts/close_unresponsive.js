function labeledEvent(data) {
  return (
    data.event === "labeled" && data.label.name === "more-information-needed"
  );
}

const numberOfDaysLimit = 30;
const close_message = `This has been closed since a request for information has \
not been answered for ${numberOfDaysLimit} days. It can be reopened when the \
requested information is provided.`;

module.exports = async ({ github, context }) => {
  const owner = context.repo.owner;
  const repo = context.repo.repo;

  const issues = await github.rest.issues.listForRepo({
    owner: owner,
    repo: repo,
    labels: "more-information-needed",
  });
  const numbers = issues.data.map((e) => e.number);

  for (const number of numbers) {
    const events = await github.paginate(
      github.rest.issues.listEventsForTimeline,
      {
        owner: owner,
        repo: repo,
        issue_number: number,
      },
      (response) => response.data.filter(labeledEvent),
    );

    const latest_response_label = events[events.length - 1];

    const created_at = new Date(latest_response_label.created_at);
    const now = new Date();
    const diff = now - created_at;
    const diffDays = diff / (1000 * 60 * 60 * 24);

    if (diffDays > numberOfDaysLimit) {
      github.rest.issues.update({
        owner: owner,
        repo: repo,
        issue_number: number,
        state_reason: "not_planned",
        state: "closed",
      });

      github.rest.issues.createComment({
        owner: owner,
        repo: repo,
        issue_number: number,
        body: close_message,
      });
    }
  }
};
