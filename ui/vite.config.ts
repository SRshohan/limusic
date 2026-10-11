import tailwindcss from '@tailwindcss/vite';
import adapter from '@sveltejs/adapter-static';
import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';
import { SvelteKitPWA } from '@vite-pwa/sveltekit';
import type { Plugin } from 'vite';

// No @types/node dependency for this one read; vite.config.ts always runs under Node.
declare const process: { env: Record<string, string | undefined> };

// `PwaUpdatePrompt.svelte` statically imports `virtual:pwa-register/svelte`, which only exists
// while the SvelteKitPWA plugin below is registered. It's never *mounted* in Tauri (+layout.svelte
// only renders it outside `isTauri`), but Vite still has to resolve the module graph at build time
// regardless of runtime branching — so the tauri build needs a no-op stand-in for that one import,
// never actually called since the component it lives in is never instantiated there.
function pwaRegisterStub(): Plugin {
	const id = 'virtual:pwa-register/svelte';
	const resolvedId = '\0' + id;
	return {
		name: 'limusic-pwa-register-stub',
		resolveId: (requested) => (requested === id ? resolvedId : undefined),
		load: (requested) =>
			requested === resolvedId
				? 'export function useRegisterSW() { return { needRefresh: { subscribe: () => () => {} }, updateServiceWorker: async () => {} }; }'
				: undefined
	};
}

// `pnpm build`/`pnpm dev` (mode 'development'/'production', the default) target the PWA/browser
// build and include the service worker. `pnpm build:tauri`/`pnpm dev:tauri` (--mode tauri, wired
// into src-tauri/tauri.conf.json's beforeBuildCommand/beforeDevCommand) drop the PWA plugin
// entirely: the desktop app never serves a manifest or service worker, so it has no use for one
// bundled inside it, and a webview dev session never risks registering one against localhost:5183.
export default defineConfig(({ mode }) => {
	const isTauri = mode === 'tauri';

	return {
		server: {
			port: 5183,
			strictPort: true
		},
		plugins: [
			{
				name: 'limusic-dev-referrer-policy',
				configureServer(server) {
					server.middlewares.use((_req, res, next) => {
						res.setHeader('Referrer-Policy', 'no-referrer');
						next();
					});
				}
			},
			tailwindcss(),
			sveltekit({
				compilerOptions: {
					// Force runes mode for the project, except for libraries. Can be removed in svelte 6.
					runes: ({ filename }) => filename.split(/[/\\]/).includes('node_modules') ? undefined : true
				},
				// SPA fallback so the dynamic /playlist/[id] route resolves on direct load. context/18.
				// Separate output folders: `ui/build` is what tauri.conf.json's `frontendDist`
				// embeds into the desktop app; `ui/web` is the PWA/browser build, so building one
				// never clobbers the other and both can exist on disk at once.
				adapter: adapter({ fallback: 'index.html', pages: isTauri ? 'build' : 'web' })
			}),
			isTauri && pwaRegisterStub(),
			// Installable PWA shell. Precaches the built app only — never the YouTube stream/video
			// proxy routes or the websocket, which are short-lived and not meaningfully cacheable.
			// Local-music playback never goes through the network at all (blob/File-backed), so it
			// already works offline without the service worker's help.
			!isTauri &&
				SvelteKitPWA({
					registerType: 'prompt',
					// No auto-injected register script: the service worker is registered manually (see
					// PwaUpdatePrompt.svelte) and only outside the Tauri webview, where a SW is unwanted
					// and untested against the desktop build.
					injectRegister: false,
					manifest: {
						name: 'Limusic',
						short_name: 'Limusic',
						description: 'Ad-free music, self-hosted and installable.',
						start_url: '/',
						scope: '/',
						display: 'standalone',
						background_color: '#0b0b0f',
						theme_color: '#0b0b0f',
						categories: ['music', 'entertainment'],
						icons: [
							{ src: '/icons/pwa-192.png', sizes: '192x192', type: 'image/png' },
							{ src: '/icons/pwa-512.png', sizes: '512x512', type: 'image/png' }
						]
					},
					workbox: {
						globPatterns: ['**/*.{js,css,html,svg,png,woff2}'],
						// API data is fine to serve stale-while-revalidate; media/video/ws must never be
						// intercepted by the SW (signed, short-lived, and streaming doesn't fit the cache API).
						navigateFallbackDenylist: [/^\/media\//, /^\/ws/],
						runtimeCaching: [
							{
								urlPattern: /^\/api\//,
								handler: 'NetworkFirst',
								options: { cacheName: 'api-cache', networkTimeoutSeconds: 4 }
							}
						]
					},
					devOptions: {
						// Off by default: a dev-mode SW is scoped to the origin (localhost:5183), not to
						// this checkout, so it keeps intercepting that port for whatever else runs there
						// next. Opt in with `LIMUSIC_PWA_DEV=1 pnpm dev` when actually testing offline/install
						// behavior, and unregister it afterwards (DevTools → Application → Service Workers).
						enabled: process.env.LIMUSIC_PWA_DEV === '1',
						type: 'module'
					}
				})
		]
	};
});
