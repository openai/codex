# Label Studio Import + Project Instructions

1. Create project: **New Project** → name it “AVC — LLM Gold Pilot”.
2. Label config: **Project → Settings → Labeling Interface** → paste the XML label config you already have (the `<View>...` choices & textarea). Save.
3. Import tasks: **Tasks → Import** → upload `labelstudio_gold_tasks.json`. Label Studio will create tasks with data and annotations (gold).
4. Set up permissions: Create a human annotator user and add them. For gold checks, duplicate tasks as necessary or use Compare tool in Label Studio.
5. QA: Use **Analytics/Inter-annotator agreement** or export tasks and compute IAB externally. (If you want, I can provide a one-page Python snippet to compute Fleiss/Kappa from annotation exports.)

## PDF + PNG export options

- **Quick (browser):** open `Allison_OnePager.html` → Print → Save as PDF. For PNG: Print to PDF then convert (or screenshot at high res).
- **Pro (CLI):** use `wkhtmltopdf Allison_OnePager.html Allison_OnePager.pdf`. For PNG use `wkhtmltoimage`.
- **Google Docs:** paste the plain-text one-pager from `offer_plaintext.txt` into a Google Doc and style quickly.
