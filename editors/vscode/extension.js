// @ts-check
// The Papyrus LSP client: starts `papyrus lsp` and hands it the `.psc` files.

const { execFile } = require("node:child_process");
const fs = require("node:fs");
const path = require("node:path");
const vscode = require("vscode");
const { LanguageClient } = require("vscode-languageclient/node");

/** The read-only documents holding the assembly of `.pex` files. */
const PEX_SCHEME = "papyrus-pex";

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

/**
 * Whether a `papyrus.toml` stands next to `file` or above it, up to `root`.
 * @param {string} file
 * @param {string} root
 */
function hasConfig(file, root) {
  let dir = path.dirname(file);
  while (dir.length >= root.length) {
    if (fs.existsSync(path.join(dir, "papyrus.toml"))) {
      return true;
    }
    if (dir === path.dirname(dir)) {
      break;
    }
    dir = path.dirname(dir);
  }
  return false;
}

/**
 * Shows "Compile this script" in the menus while the workspace has a
 * `papyrus.toml`.
 * @param {vscode.ExtensionContext} context
 */
function watchConfigs(context) {
  const update = async () => {
    const found = await vscode.workspace.findFiles("**/papyrus.toml", "**/node_modules/**", 1);
    await vscode.commands.executeCommand("setContext", "papyrus.hasConfig", found.length > 0);
  };
  const watcher = vscode.workspace.createFileSystemWatcher("**/papyrus.toml");
  context.subscriptions.push(
    watcher,
    watcher.onDidCreate(update),
    watcher.onDidDelete(update),
    vscode.workspace.onDidChangeWorkspaceFolders(update),
  );
  return update();
}

/**
 * Compiles scripts, saved first, and nothing else: those selected in the
 * explorer, else the one of the active editor.
 * @param {vscode.ExtensionContext} context
 * @param {vscode.Uri} [clicked]
 * @param {vscode.Uri[]} [selected]
 */
async function compileFile(context, clicked, selected) {
  const active = vscode.window.activeTextEditor?.document;
  const uris = selected ?? (clicked ? [clicked] : active?.languageId === "papyrus" ? [active.uri] : []);
  const scripts = uris.filter((uri) => uri.scheme === "file" && uri.path.endsWith(".psc"));
  if (scripts.length === 0) {
    vscode.window.showErrorMessage("Papyrus: open a script to compile it.");
    return;
  }
  const folder = vscode.workspace.getWorkspaceFolder(scripts[0]);
  if (!folder) {
    vscode.window.showErrorMessage("Papyrus: this script is outside the workspace folders.");
    return;
  }
  const paths = scripts.map((uri) => uri.fsPath);
  if (!paths.every((file) => hasConfig(file, folder.uri.fsPath))) {
    vscode.window.showErrorMessage("Papyrus: compiling a script needs its papyrus.toml.");
    return;
  }
  const open = vscode.workspace.textDocuments;
  await Promise.all(
    open.filter((d) => paths.includes(d.uri.fsPath) && d.isDirty).map((d) => d.save()),
  );
  await vscode.tasks.executeTask(
    task(context, folder, { type: "papyrus", command: "build", args: paths }),
  );
}

/**
 * The assembly of a `.pex`, or comments saying why there is none: this is
 * what the reader sees either way.
 * @param {vscode.ExtensionContext} context
 * @param {vscode.Uri} uri
 * @returns {Promise<string>}
 */
function disassemble(context, uri) {
  const command = serverPath(context);
  const file = uri.with({ scheme: "file", path: uri.path.slice(0, -".pas".length) }).fsPath;
  return new Promise((resolve) => {
    const options = { maxBuffer: 256 * 1024 * 1024 };
    execFile(command, ["disassemble", file], options, (error, stdout, stderr) => {
      resolve(
        error
          ? `; Papyrus could not disassemble this file.\n; ${stderr.trim() || error.message}\n`
          : stdout,
      );
    });
  });
}

/**
 * Opens a `.pex` as a read-only text document. A custom editor is what VSCode
 * opens a binary file with: this one hands over to the document and leaves.
 * @param {vscode.ExtensionContext} context
 */
function registerPexEditor(context) {
  const changes = new vscode.EventEmitter();
  const watcher = vscode.workspace.createFileSystemWatcher("**/*.pex");
  const assembly = (/** @type {vscode.Uri} */ uri) =>
    uri.with({ scheme: PEX_SCHEME, path: `${uri.path}.pas` });
  context.subscriptions.push(
    changes,
    watcher,
    watcher.onDidChange((uri) => changes.fire(assembly(uri))),
    vscode.workspace.registerTextDocumentContentProvider(PEX_SCHEME, {
      onDidChange: changes.event,
      provideTextDocumentContent: (uri) => disassemble(context, uri),
    }),
    vscode.window.registerCustomEditorProvider("papyrus.pex", {
      openCustomDocument: (uri) => ({ uri, dispose() {} }),
      resolveCustomEditor: async (document, panel) => {
        const column = panel.viewColumn;
        await vscode.window.showTextDocument(assembly(document.uri), { viewColumn: column });
        panel.dispose();
      },
    }),
  );
}

/** @param {vscode.ExtensionContext} context */
async function activate(context) {
  registerPexEditor(context);
  await watchConfigs(context);
  context.subscriptions.push(
    vscode.commands.registerCommand("papyrus.restart", async () => {
      await client?.stop();
      await start(context);
    }),
    vscode.commands.registerCommand("papyrus.createConfig", createConfig),
    vscode.commands.registerCommand("papyrus.compileFile", (clicked, selected) =>
      compileFile(context, clicked, selected),
    ),
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
