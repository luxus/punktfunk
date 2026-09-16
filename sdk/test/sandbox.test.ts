// What a plugin can reach is decided entirely by this argv, so it is worth pinning: an empty home,
// its own state and token, the paths it declared — and nothing that was not asked for.
import { describe, expect, test } from "bun:test";
import {
	bwrapArgv,
	expandHome,
	type PluginManifest,
	sandboxEnv,
	sandboxProbe,
} from "../src/sandbox.js";

const paths = {
	stateDir: "/home/u/.config/punktfunk/plugin-state/demo",
	tokenFile: "/home/u/.config/punktfunk/plugin-state/demo/.plugin-token",
	socket: "/run/user/1000/punktfunk/plugin-demo.sock",
	pluginsDir: "/home/u/.config/punktfunk/plugins",
	bun: "/usr/lib/punktfunk-scripting/bun",
	runner: "/usr/share/punktfunk-scripting/runner-cli.js",
	home: "/home/u",
};

const manifest = (over: Partial<PluginManifest> = {}): PluginManifest => ({
	schema: 1,
	id: "demo",
	reads: ["~/.local/share/Steam"],
	...over,
});

/** `--flag a b` → the pairs for that flag. */
const binds = (argv: string[], flag: string): Array<[string, string]> => {
	const out: Array<[string, string]> = [];
	for (let i = 0; i < argv.length; i++) {
		if (argv[i] === flag) out.push([argv[i + 1] as string, argv[i + 2] as string]);
	}
	return out;
};

describe("bwrapArgv", () => {
	test("takes away the namespaces the boundary depends on", () => {
		const argv = bwrapArgv(manifest(), paths);
		// `--unshare-all` plus a fresh /proc is what stops the same uid reaching the host process
		// through /proc/<pid>/root, /proc/<pid>/environ or kill(2).
		expect(argv).toContain("--unshare-all");
		expect(argv).toContain("--disable-userns");
		// bwrap refuses `--disable-userns` unless a user namespace is DEMANDED: `--unshare-all`
		// only tries for one. Without this pair every plugin exits 1 before it starts.
		expect(argv).toContain("--unshare-user");
		expect(argv).toContain("--die-with-parent");
		expect(argv).toContain("--clearenv");
		// …which empties the child's environment, so every value must be re-stated in the argv.
		// The spawn env does not survive it: without these the plugin has no HOME and no socket.
		const joined = argv.join(" ");
		expect(joined).toContain("--setenv HOME");
		expect(joined).toContain("--setenv PUNKTFUNK_MGMT_UNIX /run/punktfunk/host.sock");
		expect(joined).toContain("--setenv PUNKTFUNK_CONFIG_DIR /run/punktfunk");
		expect(binds(argv, "--proc")).toBeDefined();
		expect(argv.join(" ")).toContain("--proc /proc");
	});

	test("binds the plugin's own things, and the home only through what it declared", () => {
		const argv = bwrapArgv(manifest(), paths);
		expect(binds(argv, "--bind")).toContainEqual([
			paths.stateDir,
			"/run/punktfunk/plugin-state",
		]);
		expect(binds(argv, "--ro-bind")).toContainEqual([
			paths.tokenFile,
			"/run/punktfunk/plugin-token",
		]);
		expect(binds(argv, "--bind")).toContainEqual([paths.socket, "/run/punktfunk/host.sock"]);
		expect(binds(argv, "--ro-bind-try")).toContainEqual([
			"/home/u/.local/share/Steam",
			"/home/u/.local/share/Steam",
		]);
		// The home itself is never bound, so neither is ~/.ssh or ~/.config/punktfunk.
		const all = [...binds(argv, "--bind"), ...binds(argv, "--ro-bind"), ...binds(argv, "--ro-bind-try")];
		expect(all.map(([src]) => src)).not.toContain("/home/u");
		expect(all.map(([src]) => src)).not.toContain("/home/u/.config/punktfunk");
	});

	test("no network unless the manifest asked for one", () => {
		expect(bwrapArgv(manifest(), paths)).not.toContain("--share-net");
		expect(bwrapArgv(manifest({ network: true }), paths)).toContain("--share-net");
	});

	test("what the operator granted is writable, and nothing relative is bound at all", () => {
		const argv = bwrapArgv(manifest({ writes: ["/tmp/vhclient"] }), paths, ["/mnt/games"]);
		expect(binds(argv, "--bind-try")).toContainEqual(["/tmp/vhclient", "/tmp/vhclient"]);
		expect(binds(argv, "--bind-try")).toContainEqual(["/mnt/games", "/mnt/games"]);
		const relative = bwrapArgv(manifest({ reads: ["not/absolute"] }), paths);
		expect(relative.join(" ")).not.toContain("not/absolute");
	});
});

describe("sandboxEnv", () => {
	test("carries no inherited value, and points the SDK at the socket", () => {
		const env = sandboxEnv("/home/u");
		// The real home, so a `~/...` read the manifest declared is where os.homedir() looks.
		expect(env.HOME).toBe("/home/u");
		expect(env.PUNKTFUNK_CONFIG_DIR).toBe("/run/punktfunk");
		expect(env.PUNKTFUNK_MGMT_UNIX).toBe("/run/punktfunk/host.sock");
		expect(env.PUNKTFUNK_MGMT_TOKEN).toBeUndefined();
	});
});

describe("expandHome", () => {
	test("resolves ~ and leaves an absolute path alone", () => {
		expect(expandHome("~/.config/x", "/home/u")).toBe("/home/u/.config/x");
		expect(expandHome("/opt/x", "/home/u")).toBe("/opt/x");
	});
});

describe("sandboxProbe", () => {
	test("says which of the two ways it is unavailable", () => {
		expect(sandboxProbe(() => ({ status: 0 }), "darwin").ok).toBe(false);
		expect(sandboxProbe(() => ({ status: 0 }), "linux")).toEqual({ ok: true });
		// The probe must ask for what bwrapArgv asks for, or it calls a box capable that then
		// refuses every plugin.
		let probed: string[] = [];
		sandboxProbe((_cmd, args) => {
			probed = args;
			return { status: 0 };
		}, "linux");
		expect(probed).toContain("--unshare-user");
		expect(probed).toContain("--disable-userns");
		const missing = sandboxProbe(() => ({ status: null }), "linux");
		expect(missing.ok).toBe(false);
		expect(!missing.ok && missing.reason).toContain("bubblewrap");
		const denied = sandboxProbe(() => ({ status: 1 }), "linux");
		expect(!denied.ok && denied.reason).toContain("user namespaces");
	});
});
