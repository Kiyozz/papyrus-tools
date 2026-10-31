// @ts-check
// The extension in a real VSCode, on `tests/fixture`: `Hello.psc` declares
// `count` twice, `PAP019 locals/duplicate`. See `.vscode-test.mjs`.

const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const vscode = require("vscode");

const folder = (vscode.workspace.workspaceFolders ?? [])[0].uri.fsPath;
const script = vscode.Uri.file(path.join(folder, "Hello.psc"));
const config = path.join(folder, "papyrus.toml");

/**
 * Polls `check` until it returns something truthy, and returns it.
 * @template T
 * @param {string} what
 * @param {() => T | undefined | Promise<T | undefined>} check
 */
async function until(what, check, ms = 30_000) {
  const end = Date.now() + ms;
  while (Date.now() < end) {
    const found = await check();
    if (found) {
      return found;
    }
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  throw new Error(`timed out waiting for ${what}`);
}

/**
 * The diagnostics of `Hello.psc` with this code. The server sends `PAP019`,
 * the problem matcher of the tasks reads `PAP019 locals/duplicate`.
 * @param {string} code
 */
function diagnostics(code) {
  return vscode.languages
    .getDiagnostics(script)
    .filter((d) => (typeof d.code === "object" ? d.code.value : d.code) === code);
}

/**
 * Runs a task, resolves with the exit code of its process.
 * @param {vscode.Task} task
 * @returns {Promise<number | undefined>}
 */
function run(task) {
  return new Promise((resolve) => {
    const listener = vscode.tasks.onDidEndTaskProcess((event) => {
      if (event.execution.task.name === task.name) {
        listener.dispose();
        resolve(event.exitCode);
      }
    });
    vscode.tasks.executeTask(task);
  });
}

/** @param {string} name */
async function papyrusTask(name) {
  const tasks = await vscode.tasks.fetchTasks({ type: "papyrus" });
  const task = tasks.find((t) => t.name === name);
  assert.ok(task, `no papyrus task ${name}`);
  return task;
}

describe("the extension", () => {
  before(async () => {
    await vscode.window.showTextDocument(script);
  });

  after(() => {
    fs.rmSync(config, { force: true });
  });

  it("activates on a .psc", async () => {
    const extension = vscode.extensions.getExtension("kiyozz.papyrus-tools");
    assert.ok(extension);
    await until("the activation", () => extension.isActive);
  });

  it("shows the diagnostics of the server", async () => {
    const [found] = await until("PAP019 from the server", () => {
      const found = diagnostics("PAP019");
      return found.length ? found : undefined;
    });
    assert.equal(found.source, "papyrus");
    assert.equal(found.severity, vscode.DiagnosticSeverity.Error);
    assert.equal(found.range.start.line, 4);
  });

  it("shows the type hierarchy", async () => {
    const items = await until("the type hierarchy", async () => {
      /** @type {vscode.TypeHierarchyItem[]} */
      const found = await vscode.commands.executeCommand(
        "vscode.prepareTypeHierarchy",
        script,
        new vscode.Position(0, 12),
      );
      return found?.length ? found : undefined;
    });
    assert.equal(items[0].name, "Hello");
  });

  it("moves a renamed script for good, even saved and formatted", async () => {
    const settings = vscode.workspace.getConfiguration("editor");
    await settings.update("formatOnSave", true, vscode.ConfigurationTarget.Workspace);
    const oldPath = path.join(folder, "RenameMe.psc");
    const newPath = path.join(folder, "Renamed.psc");
    const user = path.join(folder, "RenameUser.psc");
    fs.writeFileSync(oldPath, "ScriptName RenameMe\n");
    fs.writeFileSync(user, "ScriptName RenameUser extends RenameMe\n");
    try {
      // From where it is extended: the script itself is not open.
      const document = await vscode.workspace.openTextDocument(user);
      await vscode.window.showTextDocument(document);
      const position = new vscode.Position(0, 33);
      const found = await until("the server to know both scripts", async () => {
        try {
          /** @type {vscode.WorkspaceEdit | undefined} */
          const edit = await vscode.commands.executeCommand(
            "vscode.executeDocumentRenameProvider",
            document.uri,
            position,
            "Renamed",
          );
          return edit?.size === 2 ? edit : undefined;
        } catch {
          return undefined;
        }
      });
      // In the server's order: this command files them in another.
      const edit = new vscode.WorkspaceEdit();
      const oldUri = vscode.Uri.file(oldPath);
      const newUri = vscode.Uri.file(newPath);
      for (const [uri, edits] of found.entries()) {
        for (const { range, newText } of edits) {
          edit.replace(uri, range, newText);
        }
      }
      edit.renameFile(oldUri, newUri);
      assert.ok(await vscode.workspace.applyEdit(edit, { isRefactoring: true }));
      await until("the new file", () => fs.existsSync(newPath));
      await new Promise((resolve) => setTimeout(resolve, 1500));
      // `files.autoSave: onWindowChange` saved the stale copy.
      await vscode.workspace.saveAll(true);
      await new Promise((resolve) => setTimeout(resolve, 500));
      assert.ok(!fs.existsSync(oldPath), "the old file came back");
      assert.match(fs.readFileSync(user, "utf8"), /extends Renamed/i);
    } finally {
      await vscode.commands.executeCommand("workbench.action.closeAllEditors");
      await settings.update("formatOnSave", undefined, vscode.ConfigurationTarget.Workspace);
      for (const file of [oldPath, newPath, user]) {
        fs.rmSync(file, { force: true });
      }
      fs.rmSync(path.join(folder, ".vscode"), { recursive: true, force: true });
    }
  });

  it("provides the build and check tasks", async () => {
    const tasks = await vscode.tasks.fetchTasks({ type: "papyrus" });
    assert.deepEqual(tasks.map((t) => t.name).sort(), ["build", "check"]);
    const build = tasks.find((t) => t.name === "build");
    assert.equal(build?.group?.id, vscode.TaskGroup.Build.id);
  });

  it("puts the problems of check in the Problems view", async () => {
    assert.equal(await run(await papyrusTask("check")), 1);
    const [found] = await until("PAP019 from the task", () => {
      const found = diagnostics("PAP019 locals/duplicate");
      return found.length ? found : undefined;
    });
    assert.equal(found.range.start.line, 4);
  });

  it("runs build in the folder", async () => {
    // No `game` and no compiler in the fixture: `papyrus build` stops there.
    assert.equal(await run(await papyrusTask("build")), 2);
  });

  it("creates papyrus.toml", async () => {
    assert.ok(!fs.existsSync(config));
    await vscode.commands.executeCommand("papyrus.createConfig");
    assert.match(fs.readFileSync(config, "utf8"), /^#:schema /);
    assert.equal(vscode.window.activeTextEditor?.document.uri.fsPath, vscode.Uri.file(config).fsPath);
  });
});
