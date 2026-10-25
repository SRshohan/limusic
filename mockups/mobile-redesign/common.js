// Shared behavior across every mockup page: dark/light toggle (persisted) and the scroll-rail
// edge-fade (same logic as the real app's ChipRail.svelte, validated separately in that work).

(function initTheme() {
	const stored = localStorage.getItem('limusic-mockup-theme');
	const theme = stored || (matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light');
	document.documentElement.setAttribute('data-theme', theme);
})();

function toggleTheme() {
	const next = document.documentElement.getAttribute('data-theme') === 'dark' ? 'light' : 'dark';
	document.documentElement.setAttribute('data-theme', next);
	localStorage.setItem('limusic-mockup-theme', next);
	document.querySelectorAll('.theme-toggle').forEach((btn) => (btn.textContent = next === 'dark' ? '☀️' : '🌙'));
}

document.addEventListener('DOMContentLoaded', () => {
	document.querySelectorAll('.theme-toggle').forEach((btn) => {
		btn.textContent = document.documentElement.getAttribute('data-theme') === 'dark' ? '☀️' : '🌙';
		btn.addEventListener('click', toggleTheme);
	});

	document.querySelectorAll('.rail-wrap').forEach((wrap) => {
		const rail = wrap.querySelector('.rail');
		const left = wrap.querySelector('.fade.left');
		const right = wrap.querySelector('.fade.right');
		if (!rail) return;
		function update() {
			const atStart = rail.scrollLeft <= 1;
			const atEnd = rail.scrollLeft + rail.clientWidth >= rail.scrollWidth - 1;
			if (left) left.classList.toggle('visible', !atStart);
			if (right) right.classList.toggle('visible', !atEnd);
		}
		rail.addEventListener('scroll', update);
		new ResizeObserver(update).observe(rail);
		update();
	});
});
