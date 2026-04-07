/**
 * Resizable table columns — pure DOM approach with localStorage persistence.
 * Usage: call `initResizableColumns(tableEl, 'unique-table-id')` after mount.
 * Adds drag handles to every <th> in the first <thead><tr>.
 * Column widths are saved to localStorage and restored on next visit.
 * Narrow columns (checkboxes, icons) are detected and locked automatically.
 */
export function initResizableColumns(table: HTMLTableElement, tableId?: string) {
	const headerRow = table.querySelector('thead tr');
	if (!headerRow) return;

	// Prevent double-init
	if (table.dataset.resizable === 'true') return;
	table.dataset.resizable = 'true';

	// Auto-generate tableId from page path if not provided
	const id = tableId || `col-widths:${location.pathname}`;

	// Force table-layout fixed for predictable column widths
	table.style.tableLayout = 'fixed';

	const ths = Array.from(headerRow.querySelectorAll('th')) as HTMLThElement[];

	// Detect narrow/fixed columns (checkboxes, action icons) — skip resizing for these
	const isFixed = ths.map(th => {
		const w = th.getBoundingClientRect().width;
		// Columns narrower than 50px or with w-10 class are fixed
		return w < 50 || th.classList.contains('w-10');
	});

	// Try to restore saved widths (only for resizable columns)
	const saved = loadWidths(id, ths.length);

	ths.forEach((th, i) => {
		const isLast = i === ths.length - 1;
		if (isLast) {
			// Last column takes remaining space — no fixed width
			th.style.width = '';
		} else if (isFixed[i]) {
			// Lock fixed columns to their current width
			th.style.width = th.getBoundingClientRect().width + 'px';
		} else if (saved && saved[i]) {
			th.style.width = saved[i] + 'px';
		} else {
			th.style.width = th.getBoundingClientRect().width + 'px';
		}
		th.style.position = 'relative';
	});

	// Add resize handles only between two resizable columns
	ths.forEach((th, i) => {
		if (i === ths.length - 1) return;
		// Skip handle if this column or next column is fixed
		if (isFixed[i] || isFixed[i + 1]) return;

		const handle = document.createElement('div');
		handle.className = 'col-resize-handle';
		handle.addEventListener('mousedown', (e) => startResize(e, th, ths[i + 1], id, ths, isFixed));
		handle.addEventListener('click', (e) => { e.preventDefault(); e.stopPropagation(); });
		handle.addEventListener('dblclick', (e) => { e.stopPropagation(); autoFit(table, i, id, ths, isFixed); });
		th.appendChild(handle);
	});
}

function saveWidths(id: string, ths: HTMLElement[]) {
	const widths = ths.map(th => Math.round(th.getBoundingClientRect().width));
	try {
		localStorage.setItem(id, JSON.stringify(widths));
	} catch {}
}

function loadWidths(id: string, count: number): number[] | null {
	try {
		const raw = localStorage.getItem(id);
		if (!raw) return null;
		const widths = JSON.parse(raw) as number[];
		if (widths.length !== count) return null;
		return widths;
	} catch {
		return null;
	}
}

function startResize(e: MouseEvent, th: HTMLElement, nextTh: HTMLElement, id: string, ths: HTMLElement[], isFixed: boolean[]) {
	e.preventDefault();
	e.stopPropagation();

	const startX = e.clientX;
	const startW = th.getBoundingClientRect().width;
	const nextStartW = nextTh.getBoundingClientRect().width;
	const minW = 40;

	const handle = e.target as HTMLElement;
	handle.classList.add('active');

	function onMove(ev: MouseEvent) {
		const dx = ev.clientX - startX;
		const newW = Math.max(minW, startW + dx);
		const newNextW = Math.max(minW, nextStartW - dx);
		if (newW >= minW && newNextW >= minW) {
			th.style.width = newW + 'px';
			nextTh.style.width = newNextW + 'px';
		}
	}

	function onUp() {
		handle.classList.remove('active');
		document.removeEventListener('mousemove', onMove);
		document.removeEventListener('mouseup', onUp);
		saveWidths(id, ths);
	}

	document.addEventListener('mousemove', onMove);
	document.addEventListener('mouseup', onUp);
}

function autoFit(table: HTMLTableElement, colIndex: number, id: string, ths: HTMLElement[], isFixed: boolean[]) {
	if (isFixed[colIndex]) return;
	const rows = table.querySelectorAll('tbody tr');
	let maxW = 60;
	rows.forEach(row => {
		const cells = row.querySelectorAll('td');
		if (cells[colIndex]) {
			const content = cells[colIndex].scrollWidth;
			if (content > maxW) maxW = content;
		}
	});
	if (ths[colIndex]) {
		ths[colIndex].style.width = Math.min(maxW + 16, 600) + 'px';
		saveWidths(id, ths);
	}
}
