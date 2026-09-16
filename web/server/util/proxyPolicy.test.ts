import { describe, expect, test } from "bun:test";
import { requiresPasswordConfirmation } from "./proxyPolicy";

const confirmedRoutes = [
	["POST", "/api/v1/update/apply"],
	["POST", "/api/v1/store/install"],
	["POST", "/api/v1/actions/restart"],
	["POST", "/api/v1/native/pair/arm"],
	["POST", "/api/v1/native/pending/7/approve"],
	["POST", "/api/v1/pair/pin"],
	["POST", "/api/v1/library/custom"],
	["PUT", "/api/v1/store/sources/community"],
	["PUT", "/api/v1/hooks"],
	["PUT", "/api/v1/library/custom/game"],
	["PUT", "/api/v1/library/provider/scanner"],
] as const;

describe("password-confirmed proxy routes", () => {
	test("covers every dedicated confirmation handler", () => {
		for (const [method, path] of confirmedRoutes) {
			expect(requiresPasswordConfirmation(method, path)).toBe(true);
		}
	});

	test("covers normalized aliases that fall through to the proxy", () => {
		const aliases = [
			["POST", "/api//v1/update/apply"],
			["PUT", "/api/./v1/hooks"],
			["POST", "/api/v1/store/%69nstall"],
			["PUT", "/api/v1/library/x/../custom/game"],
		] as const;
		for (const [method, path] of aliases) {
			expect(requiresPasswordConfirmation(method, path)).toBe(true);
		}
	});

	test("does not block nearby routes or other methods", () => {
		const ordinaryRoutes = [
			["GET", "/api/v1/update/apply"],
			["POST", "/api/v1/update/check"],
			["POST", "/api/v1/actions"],
			["PUT", "/api/v1/store/source/community"],
			["DELETE", "/api/v1/library/custom/game"],
		] as const;
		for (const [method, path] of ordinaryRoutes) {
			expect(requiresPasswordConfirmation(method, path)).toBe(false);
		}
	});
});
