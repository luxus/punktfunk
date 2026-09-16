import { normalizePath } from "./auth";

const passwordConfirmedRoutes: Record<string, RegExp[]> = {
	POST: [
		/^\/api\/v1\/update\/apply\/?$/,
		/^\/api\/v1\/store\/install\/?$/,
		/^\/api\/v1\/actions\/[^/]+\/?$/,
		/^\/api\/v1\/native\/pair\/arm\/?$/,
		/^\/api\/v1\/native\/pending\/[^/]+\/approve\/?$/,
		/^\/api\/v1\/pair\/pin\/?$/,
		/^\/api\/v1\/library\/custom\/?$/,
	],
	PUT: [
		/^\/api\/v1\/store\/sources\/[^/]+\/?$/,
		/^\/api\/v1\/hooks\/?$/,
		/^\/api\/v1\/library\/custom\/[^/]+\/?$/,
		/^\/api\/v1\/library\/provider\/[^/]+\/?$/,
	],
};

/** True when the management mutation belongs to an exact route that rechecks
 * the console password. The generic proxy refuses its normalized aliases so
 * they cannot reach the host with only a session cookie. */
export function requiresPasswordConfirmation(
	method: string | undefined,
	pathname: string,
): boolean {
	const routes = passwordConfirmedRoutes[method?.toUpperCase() ?? ""];
	if (!routes) return false;
	const normalized = normalizePath(pathname);
	return routes.some((route) => route.test(normalized));
}
