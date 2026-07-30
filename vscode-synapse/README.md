# Synapse VS Code Support

This is a small local VS Code extension for `.syn` files. It provides syntax
highlighting and basic editor behavior for the Synapse DSL, including Synapse
0.3 command groups, telemetry declarations, `@cc(...)` attributes, and
`//`/`///` comments.

## Try It Locally

From the repo root:

```bash
code --extensionDevelopmentPath="$PWD/vscode-synapse" "$PWD"
```

Then open a `.syn` file. VS Code should identify it as `Synapse`.

## Test

The grammar checks use Node's built-in test runner and require no installed
packages:

```bash
npm test --prefix vscode-synapse
```

## Package Later

If this grows beyond basic highlighting, it can be packaged with `vsce` and published or installed as a `.vsix`.
