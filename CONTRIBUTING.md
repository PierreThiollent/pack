# Contributing

## Commit messages

This repository uses [Cocogitto](https://docs.cocogitto.io/) to validate commit messages with the Conventional Commits format.

Expected format:

```text
<type>(<optional scope>): <description>
```

Examples:

```text
feat(config): add retention setting
fix(storage): retry failed cleanup
chore: update release workflow
```

Common types:

- `feat`: new feature
- `fix`: bug fix
- `docs`: documentation
- `refactor`: refactoring without functional change
- `test`: tests
- `chore`: maintenance
- `ci`: continuous integration
- `build`: build, dependencies, packaging

## Installing the Git hook

Install Cocogitto:

```bash
cargo install cocogitto
```

Enable the repository hooks:

```bash
git config core.hooksPath .githooks
```

The `commit-msg` hook will then automatically run:

```bash
cog verify --file <commit-message-file>
```
