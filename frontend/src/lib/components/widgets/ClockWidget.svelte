<script lang="ts">
	import { onMount, onDestroy } from 'svelte';

	let time = $state('');
	let date = $state('');
	let interval: ReturnType<typeof setInterval>;

	function updateClock() {
		const tz = typeof localStorage !== 'undefined' ? localStorage.getItem('dp_timezone') : null;
		const now = new Date();
		const opts: Intl.DateTimeFormatOptions = {
			hour: '2-digit',
			minute: '2-digit',
			second: '2-digit',
			hour12: false,
			...(tz ? { timeZone: tz } : {}),
		};
		const dateOpts: Intl.DateTimeFormatOptions = {
			weekday: 'long',
			year: 'numeric',
			month: 'long',
			day: 'numeric',
			...(tz ? { timeZone: tz } : {}),
		};
		time = now.toLocaleTimeString(undefined, opts);
		date = now.toLocaleDateString(undefined, dateOpts);
	}

	onMount(() => {
		updateClock();
		interval = setInterval(updateClock, 1000);
	});

	onDestroy(() => { clearInterval(interval); });
</script>

<div class="flex flex-col items-center justify-center h-full p-4">
	<div class="text-3xl font-bold text-[var(--text)] tracking-wide font-mono tabular-nums">{time}</div>
	<div class="text-xs text-[var(--text-secondary)] mt-2">{date}</div>
</div>
