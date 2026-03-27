import adapter from '@sveltejs/adapter-static';

/** @type {import('@sveltejs/kit').Config} */
const config = {
	kit: {
		adapter: adapter({
			pages: 'build',
			assets: 'build',
			fallback: '200.html',
			precompress: false,
			strict: false
		}),
		prerender: {
			handleUnseenRoutes: 'ignore'
		}
	}
};

export default config;
