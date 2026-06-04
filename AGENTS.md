Start by creating a new folder for your work with an appropriate name.

Create a notes.md file in that folder and append notes to it as you work, tracking what you tried and anything you learned along the way.

Build a README.md report at the end of the investigation.

Your final commit should include just that folder and selected items from its contents:

- The notes.md and README.md files
- Any code you wrote along the way
- If you checked out and modified an existing repo, the output of "git diff" against that modified repo saved as a file - but not a copy of the full repo
- If appropriate, any binary files you created along the way provided they are less than 2MB in size

Do NOT include full copies of code that you fetched as part of your investigation. Your final commit should include only new files you created or diffs showing changes you made to existing code.

Don't create a _summary.md file - these are added automatically after you commit your changes.

If your project includes HTML demos, link to the published GitHub Pages URL in
the README, for example:
`https://simonw.github.io/research/your-folder/demo.html`.

GitHub Pages publishing can run extra build steps for individual folders. If
your folder needs generated static assets that should be available on the
published site, add a `github-pages.sh` script in that folder. Read
`build-github-pages.sh` and `run-github-pages-hooks.sh` for details.
