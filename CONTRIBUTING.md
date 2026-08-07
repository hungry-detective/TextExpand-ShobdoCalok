# Contributing to Shobdo Calok

Thanks for wanting to contribute! Here's how to get started.

## Getting started

1. **Fork** the repository and clone your fork:
   ```bash
   git clone https://github.com/<your-username>/TextExpand-ShobdoCalok.git
   cd TextExpand-ShobdoCalok
   ```
2. **Create a virtual environment** and install dependencies:
   ```bash
   python -m venv .venv
   .venv\Scripts\activate
   pip install -r requirements.txt
   ```
3. **Run the app** to verify your setup:
   ```bash
   python textqml\main.py
   ```

## Code layout

```
textqml/
├── main.py                 # Entry point, wires everything together
├── main.qml                # Top-level QML window
├── version.py              # Central version + GitHub repo config
├── Components/             # QML UI components (pages, widgets)
├── viewmodels/             # Python <-> QML bridge (snippet engine, drive, updater)
├── framelesswindow/        # Native frameless window effects
├── icons/                  # UI icons
└── ShobdoCalok.spec        # PyInstaller build spec
```

## Architecture notes

- **QML is the UI**, Python viewmodels are the backend. New UI work goes in `Components/`.
- Viewmodels are exposed to QML as context properties (`snippetViewModel`, `driveViewModel`, `updaterViewModel`) — see `main.py`.
- App data is stored next to the EXE in `AppData\` (portable). **Never** hard-code absolute paths.
- Never commit secrets. `client_secret.json` and `google_token.json` are git-ignored for a reason.

## Making changes

- Follow the existing code style (4-space indent, snake_case in Python, camelCase in QML).
- Keep changes focused. One feature or fix per PR.
- If you touch expansion behaviour, update the boundary tests in `_test_engine.py`.

## Versioning & releases

- The current version lives in `textqml/version.py` (`APP_VERSION`).
- Maintainers create releases by pushing tags (`git tag v1.1.0 && git push origin v1.1.0`).
- GitHub Actions builds and attaches the portable ZIP automatically.

## Submitting a pull request

1. Create a branch: `git checkout -b fix/my-fix`
2. Commit your changes with a clear message.
3. Push and open a PR against `main`.
4. In the PR description, explain what and why.

## Reporting bugs

Open an issue with:
- Shobdo Calok version (About page)
- Windows version
- Steps to reproduce
- Expected vs. actual behaviour

## Questions?

Open an issue — we're happy to help.
