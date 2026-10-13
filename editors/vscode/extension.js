// @ts-check
// The Papyrus LSP client: starts `papyrus lsp` and hands it the `.psc` files.

const fs = require("node:fs");
const path = require("node:path");
const vscode = require("vscode");
const { LanguageClient } = require("vscode-languageclient/node");

/** @type {LanguageClient | undefined} */
let client;

/**
 * The binary to start: the setting, else the one bundled with the extension,
 * else `papyrus` from the `PATH`.
 * @param {vscode.ExtensionContext} context
 */
function serverPath(context) {
  const configured = vscode.workspace.getConfiguration("papyrus").get("server.path");
  if (typeof configured === "string" && configured.trim() !== "") {
    return configured;
  }
  const exe = process.platform === "win32" ? "papyrus.exe" : "papyrus";
  const bundled = path.join(context.extensionPath, "server", exe);
  if (!fs.existsSync(bundled)) {
    return "papyrus";
  }
  if (process.platform !== "win32") {
    // A `.vsix` does not guarantee the executable bit.
    try {
      fs.chmodSync(bundled, 0o755);
    } catch {
      // Read-only: never mind, launching will say why.
    }
  }
  return bundled;
}

/** @param {vscode.ExtensionContext} context */
async function start(context) {
  const command = serverPath(context);
  client = new LanguageClient(
    "papyrus",
    "Papyrus",
    { command, args: ["lsp"] },
    {
      documentSelector: [{ scheme: "file", language: "papyrus" }],
      initializationOptions: {
        inlayHints: {
          parameterNames: vscode.workspace
            .getConfiguration("papyrus")
            .get("inlayHints.parameterNames"),
        },
      },
      // Changes to the `papyrus` settings reach the server.
      synchronize: { configurationSection: "papyrus" },
    },
  );
  try {
    await client.start();
  } catch (error) {
    vscode.window.showErrorMessage(
      `Papyrus: could not start "${command} lsp". ` +
        "Install papyrus, or set papyrus.server.path.",
    );
    throw error;
  }
}

/** @param {vscode.ExtensionContext} context */
async function activate(context) {
  context.subscriptions.push(
    vscode.commands.registerCommand("papyrus.restart", async () => {
      await client?.stop();
      await start(context);
    }),
    // The reference lenses: the server sends LSP types, which
    // `editor.action.showReferences` wants as VSCode's.
    vscode.commands.registerCommand("papyrus.showReferences", (uri, position, locations) => {
      if (!client) {
        return;
      }
      const convert = client.protocol2CodeConverter;
      return vscode.commands.executeCommand(
        "editor.action.showReferences",
        convert.asUri(uri),
        convert.asPosition(position),
        locations.map((/** @type {any} */ location) => convert.asLocation(location)),
      );
    }),
  );
  await start(context);
}

async function deactivate() {
  await client?.stop();
}

module.exports = { activate, deactivate };
