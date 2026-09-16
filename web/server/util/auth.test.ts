// The post-login redirect target. The gate hands a signed-in visitor on from /login to this path,
// so it is what stands between a crafted `?next=` link and an open redirect.
import { beforeEach, describe, expect, test } from "bun:test";
import { mkdtempSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { authConfigured, safeNextPath, verifyUiPassword } from "./auth";

describe("safeNextPath", () => {
	test("keeps a same-origin path with its query and hash", () => {
		expect(safeNextPath("/host?tab=power#top")).toBe("/host?tab=power#top");
	});

	test("falls back to / when unset or empty", () => {
		expect(safeNextPath(undefined)).toBe("/");
		expect(safeNextPath("")).toBe("/");
	});

	test("refuses an off-origin target", () => {
		for (const evil of [
			"https://evil.com/",
			"//evil.com/x",
			"/\\evil.com",
			"\\\\evil.com",
		]) {
			expect(safeNextPath(evil)).toBe("/");
		}
	});

	test("refuses the login page, which would bounce through the gate", () => {
		expect(safeNextPath("/login")).toBe("/");
		expect(safeNextPath("/login?next=%2Flogin")).toBe("/");
	});
});

// The password gate itself: a stored argon2id hash is what a login is checked against, and the
// legacy clear-text line is migrated into one the first time it verifies. Order matters — the
// migration runs once per process, so only one test here may trigger it.
describe("verifyUiPassword", () => {
	const dir = mkdtempSync(join(tmpdir(), "pf-pw-"));
	const file = join(dir, "web.env");
	const epoch = join(dir, "epoch");

	beforeEach(() => {
		process.env.PUNKTFUNK_UI_PASSWORD_FILE = file;
		process.env.PUNKTFUNK_UI_EPOCH_FILE = epoch;
		delete process.env.PUNKTFUNK_UI_PASSWORD;
		delete process.env.PUNKTFUNK_UI_PASSWORD_HASH;
	});

	test("a wrong clear-text guess neither passes nor migrates", async () => {
		writeFileSync(file, "PUNKTFUNK_UI_PASSWORD=hunter2\n");
		process.env.PUNKTFUNK_UI_PASSWORD = "hunter2";
		expect(await verifyUiPassword("hunter3")).toBe(false);
		expect(readFileSync(file, "utf8")).toContain(
			"PUNKTFUNK_UI_PASSWORD=hunter2",
		);
	});

	test("the first good clear-text login replaces the line with a hash", async () => {
		writeFileSync(
			file,
			"PUNKTFUNK_UI_SECRET=keepme\nPUNKTFUNK_UI_PASSWORD=hunter2\n",
		);
		process.env.PUNKTFUNK_UI_PASSWORD = "hunter2";
		expect(await verifyUiPassword("hunter2")).toBe(true);
		const body = readFileSync(file, "utf8");
		expect(body).toContain("PUNKTFUNK_UI_SECRET=keepme"); // other keys survive
		expect(body).not.toContain("PUNKTFUNK_UI_PASSWORD=");
		expect(statSync(file).mode & 0o777).toBe(0o600);
		const hash = body.match(/^PUNKTFUNK_UI_PASSWORD_HASH='(.+)'$/m)?.[1] ?? "";
		expect(await Bun.password.verify("hunter2", hash)).toBe(true);
		// A password just set revokes what came before it, so a reset ends old sessions too.
		expect(readFileSync(epoch, "utf8")).toBe("2");
	});

	test("a stored hash accepts only its own password", async () => {
		process.env.PUNKTFUNK_UI_PASSWORD_HASH = `'${await Bun.password.hash(
			"hunter2",
			{
				algorithm: "argon2id",
			},
		)}'`; // quoted on disk, so the reader has to unquote
		expect(await verifyUiPassword("hunter2")).toBe(true);
		expect(await verifyUiPassword("hunter3")).toBe(false);
		expect(authConfigured()).toBe(true);
	});

	test("a mangled hash is refused rather than trusted", async () => {
		process.env.PUNKTFUNK_UI_PASSWORD_HASH = "not-a-hash";
		expect(await verifyUiPassword("not-a-hash")).toBe(false);
	});

	test("no key at all fails closed", () => {
		expect(authConfigured()).toBe(false);
	});
});
