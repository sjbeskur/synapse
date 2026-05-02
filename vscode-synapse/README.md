# Synapse VS Code Support

This is a small local VS Code extension for `.syn` files. It provides syntax highlighting and basic editor behavior for the Synapse DSL.

## Try It Locally

From the repo root:

```bash
code --extensionDevelopmentPath="$PWD/vscode-synapse" "$PWD"
```

Then open a `.syn` file. VS Code should identify it as `Synapse`.

## Package Later

If this grows beyond basic highlighting, it can be packaged with `vsce` and published or installed as a `.vsix`.
