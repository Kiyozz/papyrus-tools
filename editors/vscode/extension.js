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

/**
 * A `papyrus build` or `papyrus check` task, its problems in the Problems
 * view.
 * @param {vscode.ExtensionContext} context
 * @param {vscode.WorkspaceFolder} folder
 * @param {{ type: string, command: string, args?: string[] }} definition
 */
function task(context, folder, definition) {
  const args = [definition.command, "--message-format", "short", ...(definition.args ?? [])];
  const execution = new vscode.ProcessExecution(serverPath(context), args, {
    cwd: folder.uri.fsPath,
  });
  const created = new vscode.Task(
    definition,
    folder,
    definition.command,
    "papyrus",
    execution,
    "$papyrus",
  );
  if (definition.command === "build") {
    created.group = vscode.TaskGroup.Build;
  }
  return created;
}

/**
 * Writes a `papyrus.toml` in a workspace folder, the server choosing its
 * target, and opens it.
 */
async function createConfig() {
  const folders = vscode.workspace.workspaceFolders ?? [];
  const folder =
    folders.length > 1 ? await vscode.window.showWorkspaceFolderPick() : folders[0];
  if (!folder) {
    if (folders.length === 0) {
      vscode.window.showErrorMessage("Papyrus: open a folder to create its papyrus.toml.");
    }
    return;
  }
  if (!client) {
    vscode.window.showErrorMessage("Papyrus: the server is not running.");
    return;
  }
  /** @type {{ uri: string, created: boolean }} */
  const result = await client.sendRequest("workspace/executeCommand", {
    command: "papyrus.server.createConfig",
    arguments: [folder.uri.toString()],
  });
  if (!result.created) {
    vscode.window.showInformationMessage(`Papyrus: ${folder.name} already has a papyrus.toml.`);
  }
  await vscode.window.showTextDocument(vscode.Uri.parse(result.uri));
}

/** @param {vscode.ExtensionContext} context */
async function activate(context) {
  context.subscriptions.push(
    vscode.commands.registerCommand("papyrus.restart", async () => {
      await client?.stop();
      await start(context);
    }),
    vscode.commands.registerCommand("papyrus.createConfig", createConfig),
    vscode.tasks.registerTaskProvider("papyrus", {
      provideTasks: () =>
        (vscode.workspace.workspaceFolders ?? []).flatMap((folder) =>
          ["build", "check"].map((command) =>
            task(context, folder, { type: "papyrus", command }),
          ),
        ),
      // A task of tasks.json: its definition, the execution added.
      resolveTask: (resolved) => {
        const scope = resolved.scope;
        const definition = /** @type {any} */ (resolved.definition);
        if (typeof scope !== "object" || typeof definition.command !== "string") {
          return undefined;
        }
        return task(context, scope, definition);
      },
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
