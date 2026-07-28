/*
* vim motions for post pages.
*
* a static page has no buffer to move a cursor through, so we build one:
* every character of every text node under <main class="post"> is measured
* once and becomes a cell — document order, plus its own box on screen.
* cells sharing a row are grouped into visual lines, which is what the eye
* sees and what j/k should follow, no matter how the markup is nested.
*
* from there every motion is just an index into a flat array, and the
* cursor is one absolutely positioned box inverting whatever sits under it.
*
* this file was written by Claude (Opus 5).
*
* copyright (c) 2026 Andrea Di Matteo — MIT, see LICENSE.
* SPDX-License-Identifier: MIT
* */
(function () {
    'use strict';

    const root     = document.querySelector('main.post');
    const cursorEl = document.getElementById('vim-cursor');
    const barEl    = document.getElementById('vim-bar');
    const helpEl   = document.getElementById('vim-help');

    if (!root || !cursorEl || !barEl || !helpEl) return;

    /* a cursor is only worth drawing where there is a keyboard */
    if (!window.matchMedia('(hover: hover) and (pointer: fine)').matches) return;

    const fileEl = document.getElementById('vim-file');
    const hintEl = document.getElementById('vim-hint');
    const msgEl  = document.getElementById('vim-msg');
    const showEl = document.getElementById('vim-show');
    const posEl  = document.getElementById('vim-pos');
    const pctEl  = document.getElementById('vim-pct');
    const signEl = document.getElementById('vim-sign');
    const cmdEl  = document.getElementById('vim-cmd');
    const cmdBox = document.getElementById('vim-cmdline');
    const helpBox = document.getElementById('vim-help-box');
    const topbar = document.querySelector('.topbar');

    const NORMAL = 0, VISUAL = 1, VLINE = 2, CMD = 3;

    const range    = document.createRange();
    const displays = new Map();

    let cells = [];      /* one entry per rendered character   */
    let lines = [];      /* index ranges into cells, per row   */
    let text  = '';      /* cells joined, for searching        */

    let pos    = 0;
    let wantX  = null;   /* sticky column for j/k              */
    let mode   = NORMAL;
    let anchor = 0;      /* other end of the visual selection  */

    let count   = '';
    let pending = '';
    let lastFind = null;
    let message = '';
    let messageKind = '';
    let msgTimer = 0;
    let hinted = false;  /* the keys hint goes away once keys are used */
    let restore = 0;     /* cursor to fall back to on Esc in cmdline  */

    let pattern = '';
    let hits    = [];
    let searchDir = 1;

    /* ------------------------------------------------------------------
    * the buffer
    * ------------------------------------------------------------------ */

    /* nearest ancestor that is not inline: the unit { and } move over */
    function blockOf(node) {
        let el = node.parentElement;

        while (el && el !== root) {
            let display = displays.get(el);

            if (display === undefined) {
                display = getComputedStyle(el).display;
                displays.set(el, display);
            }

            if (display.slice(0, 6) !== 'inline') return el;
            el = el.parentElement;
        }

        return root;
    }

    function build() {
        const previous = cells[pos];

        const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT, {
            acceptNode(node) {
                const el = node.parentElement;
                if (!node.nodeValue || !el) return NodeFilter.FILTER_REJECT;
                if (el.closest('script, style, [hidden]')) return NodeFilter.FILTER_REJECT;
                return NodeFilter.FILTER_ACCEPT;
            },
        });

        const sx = window.scrollX;
        const sy = window.scrollY;
        const out = [];
        let node;

        while ((node = walker.nextNode())) {
            const s = node.nodeValue;
            const blk = blockOf(node);

            for (let i = 0; i < s.length; i++) {
                range.setStart(node, i);
                range.setEnd(node, i + 1);

                const box = range.getBoundingClientRect();

                /*
                * collapsed whitespace between blocks, and the newlines
                * inside a <pre>, have no box: they are not characters
                * the cursor can sit on.
                * */
                if (box.width <= 0 || box.height <= 0) continue;

                out.push({
                    node: node,
                    off: i,
                    ch: s[i],
                    blk: blk,
                    x: box.left + sx,
                    y: box.top + sy,
                    w: box.width,
                    h: box.height,
                    line: 0,
                });
            }
        }

        cells = out;
        text  = out.map((c) => c.ch).join('');
        groupLines();

        /*
        * a rebuild is a remeasure, not a reload: the node the cursor sat
        * on is still there, so put it back where it was.
        * */
        pos = 0;
        if (previous) {
            for (let i = 0; i < cells.length; i++) {
                if (cells[i].node === previous.node && cells[i].off === previous.off) {
                    pos = i;
                    break;
                }
            }
        }

        anchor = Math.min(anchor, Math.max(0, cells.length - 1));
        if (pattern) research();
    }

    /*
    * two cells belong to the same visual line when their boxes overlap
    * vertically and the second one carries on to the right. a wrapped
    * line restarts at the left margin, which is exactly what breaks it.
    * */
    function groupLines() {
        lines = [];
        let line = null;

        for (let i = 0; i < cells.length; i++) {
            const c = cells[i];
            const top = c.y;
            const bottom = c.y + c.h;

            const overlap = line
                ? Math.min(line.bottom, bottom) - Math.max(line.top, top)
                : 0;

            const same = line
                && overlap > 0.5 * Math.min(line.bottom - line.top, c.h)
                && c.x + c.w > line.right - 0.5;

            if (same) {
                line.last = i;
                line.top = Math.min(line.top, top);
                line.bottom = Math.max(line.bottom, bottom);
                line.right = Math.max(line.right, c.x + c.w);
            } else {
                line = { first: i, last: i, top: top, bottom: bottom, left: c.x, right: c.x + c.w };
                lines.push(line);
            }

            c.line = lines.length - 1;
        }
    }

    /* ------------------------------------------------------------------
    * drawing
    * ------------------------------------------------------------------ */

    /*
    * measured live rather than read back from the cell: a code block
    * scrolled sideways moves its own text, and the cursor has to follow.
    * */
    function cellRect(i) {
        const c = cells[i];
        range.setStart(c.node, c.off);
        range.setEnd(c.node, c.off + 1);
        return range.getBoundingClientRect();
    }

    const barTop = () => (topbar ? topbar.getBoundingClientRect().height : 0);
    const barBottom = () => barEl.getBoundingClientRect().height;

    function draw() {
        if (!cells.length) {
            cursorEl.hidden = true;
            return;
        }

        const r = cellRect(pos);

        cursorEl.style.transform =
            'translate(' + (r.left + window.scrollX) + 'px,' + (r.top + window.scrollY) + 'px)';
        cursorEl.style.width = Math.max(r.width, r.height * 0.5) + 'px';
        cursorEl.style.height = r.height + 'px';
        cursorEl.hidden = false;

        status();
    }

    /* keep a couple of lines of air above and below, like 'scrolloff' */
    function reveal() {
        const r = cellRect(pos);
        const top = barTop();
        const pad = r.height * 2;
        const floor = window.innerHeight - barBottom() - pad;

        if (r.top < top + pad) window.scrollBy(0, r.top - top - pad);
        else if (r.bottom > floor) window.scrollBy(0, r.bottom - floor);
    }

    /* a long line inside a scrollable <pre> has to be scrolled to */
    function revealX() {
        let el = cells[pos].node.parentElement;

        while (el && el !== root) {
            const style = getComputedStyle(el);

            if (/auto|scroll/.test(style.overflowX) && el.scrollWidth > el.clientWidth) {
                const box = el.getBoundingClientRect();
                const r = cellRect(pos);

                if (r.left < box.left + 8) el.scrollLeft -= box.left + 8 - r.left;
                else if (r.right > box.right - 8) el.scrollLeft += r.right - box.right + 8;
                return;
            }

            el = el.parentElement;
        }
    }

    function setPos(i, keepColumn) {
        if (!cells.length) return;

        pos = Math.max(0, Math.min(cells.length - 1, i));
        if (!keepColumn) wantX = null;

        if (mode === VISUAL || mode === VLINE) paintSelection();

        revealX();
        reveal();
        draw();
    }

    /* ------------------------------------------------------------------
    * the status line
    * ------------------------------------------------------------------ */

    function say(s, kind) {
        message = s;
        messageKind = kind || '';

        clearTimeout(msgTimer);
        if (s) msgTimer = setTimeout(() => { message = ''; status(); }, 3500);

        status();
    }

    function percent() {
        if (lines.length <= 1) return 'All';

        const l = cells[pos].line;
        if (l === 0) return 'Top';
        if (l === lines.length - 1) return 'Bot';

        return Math.round((l / (lines.length - 1)) * 100) + '%';
    }

    function status() {
        if (!cells.length) return;

        const line = cells[pos].line;

        posEl.textContent = (line + 1) + ',' + (pos - lines[line].first + 1);
        pctEl.textContent = percent();
        showEl.textContent = count + pending;

        let left = message;
        let kind = messageKind;

        if (!left && mode === VISUAL) { left = '-- VISUAL --'; kind = 'mode'; }
        if (!left && mode === VLINE)  { left = '-- VISUAL LINE --'; kind = 'mode'; }

        msgEl.textContent = left;
        msgEl.className = 'vim-msg' + (kind ? ' is-' + kind : '');
        hintEl.hidden = hinted || left !== '';
    }

    /* ------------------------------------------------------------------
    * motions
    * ------------------------------------------------------------------ */

    const lineAt = (i) => lines[cells[i].line];
    const centerX = (i) => cells[i].x + cells[i].w / 2;

    function nearestInLine(line, x) {
        let best = line.first;
        let dist = Infinity;

        for (let i = line.first; i <= line.last; i++) {
            const d = Math.abs(centerX(i) - x);
            if (d < dist) { dist = d; best = i; }
        }

        return best;
    }

    function toLine(index, x) {
        const line = lines[Math.max(0, Math.min(lines.length - 1, index))];
        return nearestInLine(line, x === undefined ? (wantX === null ? centerX(pos) : wantX) : x);
    }

    function vertical(delta) {
        if (wantX === null) wantX = centerX(pos);
        setPos(toLine(cells[pos].line + delta), true);
    }

    function firstNonBlank(line) {
        for (let i = line.first; i <= line.last; i++) {
            if (!/\s/.test(cells[i].ch)) return i;
        }
        return line.first;
    }

    /*
    * vim's three character classes: blank, keyword, punctuation.
    * W/B/E collapse the last two into one.
    * */
    function klass(i, big) {
        const c = cells[i].ch;
        if (/\s/.test(c)) return 0;
        if (big) return 1;
        return /[A-Za-z0-9_À-ɏ]/.test(c) ? 2 : 1;
    }

    function wordForward(i, big) {
        const n = cells.length;
        let j = i;

        if (klass(j, big) !== 0) {
            const k = klass(j, big);
            while (j + 1 < n && klass(j + 1, big) === k && cells[j + 1].line === cells[j].line) j++;
            j++;
        } else {
            j++;
        }

        while (j < n && klass(j, big) === 0) j++;
        return Math.min(j, n - 1);
    }

    function wordBack(i, big) {
        let j = i - 1;
        while (j > 0 && klass(j, big) === 0) j--;
        if (j <= 0) return 0;

        const k = klass(j, big);
        while (j > 0 && klass(j - 1, big) === k && cells[j - 1].line === cells[j].line) j--;

        return j;
    }

    /* ge: the end of the previous word, not its start */
    function wordEndBack(i, big) {
        let j = i - 1;
        if (j <= 0) return 0;

        const k = klass(i, big);
        if (k !== 0) {
            while (j > 0 && klass(j, big) === k && cells[j].line === cells[i].line) j--;
        }

        while (j > 0 && klass(j, big) === 0) j--;
        return j;
    }

    function wordEnd(i, big) {
        const n = cells.length;
        let j = i + 1;

        while (j < n && klass(j, big) === 0) j++;
        if (j >= n) return n - 1;

        const k = klass(j, big);
        while (j + 1 < n && klass(j + 1, big) === k && cells[j + 1].line === cells[j].line) j++;

        return j;
    }

    function blockForward(i) {
        const blk = cells[i].blk;
        let j = i;
        while (j < cells.length && cells[j].blk === blk) j++;
        return Math.min(j, cells.length - 1);
    }

    function blockBack(i) {
        const blk = cells[i].blk;
        let start = i;
        while (start > 0 && cells[start - 1].blk === blk) start--;

        /* already sitting on the first character: skip to the block before */
        if (start < i) return start;
        if (start === 0) return 0;

        let j = start - 1;
        const previous = cells[j].blk;
        while (j > 0 && cells[j - 1].blk === previous) j--;

        return j;
    }

    /*
    * %: from a bracket to the one closing it, counting the nesting on
    * the way. from anything else, the first bracket on the line.
    * */
    const PAIRS = { '(': ')', '[': ']', '{': '}', ')': '(', ']': '[', '}': '{' };

    function matchBracket(i) {
        const line = lineAt(i);
        let at = -1;

        for (let j = i; j <= line.last; j++) {
            if (PAIRS[cells[j].ch]) { at = j; break; }
        }

        if (at < 0) return -1;

        const open = cells[at].ch;
        const close = PAIRS[open];
        const dir = ')]}'.indexOf(open) < 0 ? 1 : -1;

        let depth = 0;

        for (let j = at; j >= 0 && j < cells.length; j += dir) {
            if (cells[j].ch === open) depth++;
            else if (cells[j].ch === close) depth--;

            if (depth === 0) return j;
        }

        return -1;
    }

    function findChar(ch, dir, n) {
        const line = lineAt(pos);
        let i = pos;

        for (let hit = 0; hit < n; hit++) {
            let found = -1;

            if (dir > 0) {
                for (let j = i + 1; j <= line.last; j++) {
                    if (cells[j].ch === ch) { found = j; break; }
                }
            } else {
                for (let j = i - 1; j >= line.first; j--) {
                    if (cells[j].ch === ch) { found = j; break; }
                }
            }

            if (found < 0) { say('E486: Pattern not found: ' + ch, 'error'); return; }
            i = found;
        }

        setPos(i);
    }

    /* ------------------------------------------------------------------
    * screen motions
    * ------------------------------------------------------------------ */

    const viewTop = () => window.scrollY + barTop();
    const viewBottom = () => window.scrollY + window.innerHeight - barBottom();

    function screenLine(where) {
        const top = viewTop();
        const bottom = viewBottom();

        let first = -1;
        let last = -1;

        for (let i = 0; i < lines.length; i++) {
            if (lines[i].top >= top - 1 && lines[i].bottom <= bottom + 1) {
                if (first < 0) first = i;
                last = i;
            }
        }

        if (first < 0) return cells[pos].line;
        if (where === 'H') return first;
        if (where === 'L') return last;
        return Math.floor((first + last) / 2);
    }

    function scrollPage(px) {
        const target = cells[pos].y + px;
        window.scrollBy(0, px);

        if (wantX === null) wantX = centerX(pos);
        setPos(toLine(nearestLine(target)), true);
    }

    function nearestLine(y) {
        let best = 0;
        let dist = Infinity;

        for (let i = 0; i < lines.length; i++) {
            const d = Math.abs(lines[i].top - y);
            if (d < dist) { dist = d; best = i; }
        }

        return best;
    }

    /* ctrl-e / ctrl-y: the view moves, the cursor only if it is pushed out */
    function scrollView(px) {
        window.scrollBy(0, px);

        const r = cellRect(pos);
        const top = barTop();
        const floor = window.innerHeight - barBottom();

        if (r.top < top) setPos(toLine(nearestLine(window.scrollY + top)), true);
        else if (r.bottom > floor) setPos(toLine(nearestLine(window.scrollY + floor - r.height * 2)), true);
        else draw();
    }

    function redraw(where) {
        const line = lineAt(pos);
        const top = barTop();
        const height = window.innerHeight - top - barBottom();

        let y = line.top - top;
        if (where === 'z') y = (line.top + line.bottom) / 2 - top - height / 2;
        if (where === 'b') y = line.bottom - top - height;

        window.scrollTo(0, Math.max(0, y));
        draw();
    }

    /* ------------------------------------------------------------------
    * search
    * ------------------------------------------------------------------ */

    function compile(src) {
        /* smartcase: a lowercase pattern ignores case, a mixed one does not */
        const flags = src.toLowerCase() === src ? 'gi' : 'g';

        try {
            return new RegExp(src, flags);
        } catch (e) {
            return new RegExp(src.replace(/[.*+?^${}()|[\]\\]/g, '\\$&'), flags);
        }
    }

    function research() {
        const re = compile(pattern);
        hits = [];

        let m;
        while ((m = re.exec(text)) !== null) {
            if (m[0].length === 0) { re.lastIndex++; continue; }
            hits.push([m.index, m.index + m[0].length]);
            if (hits.length > 5000) break;
        }

        paintHits();
    }

    function paintHits() {
        if (!window.CSS || !CSS.highlights || typeof Highlight !== 'function') return;

        CSS.highlights.delete('vim-search');
        if (!hits.length) return;

        const ranges = hits.map(([from, to]) => {
            const r = document.createRange();
            r.setStart(cells[from].node, cells[from].off);
            r.setEnd(cells[to - 1].node, cells[to - 1].off + 1);
            return r;
        });

        CSS.highlights.set('vim-search', new Highlight(...ranges));
    }

    function dropHighlight() {
        hits = [];
        if (window.CSS && CSS.highlights) CSS.highlights.delete('vim-search');
    }

    function clearHits() {
        pattern = '';
        dropHighlight();
    }

    function jumpTo(dir, from, quiet) {
        if (!hits.length) {
            if (!quiet) say('E486: Pattern not found: ' + pattern, 'error');
            return false;
        }

        let target = -1;
        let wrapped = false;

        if (dir > 0) {
            for (const [start] of hits) {
                if (start > from) { target = start; break; }
            }
            if (target < 0) { target = hits[0][0]; wrapped = true; }
        } else {
            for (let i = hits.length - 1; i >= 0; i--) {
                if (hits[i][0] < from) { target = hits[i][0]; break; }
            }
            if (target < 0) { target = hits[hits.length - 1][0]; wrapped = true; }
        }

        setPos(target);

        if (wrapped && !quiet) {
            say(dir > 0
                ? 'search hit BOTTOM, continuing at TOP'
                : 'search hit TOP, continuing at BOTTOM', 'warn');
        }

        return true;
    }

    function searchWordUnderCursor(dir) {
        if (klass(pos, false) === 0) {
            say('E348: No string under cursor', 'error');
            return;
        }

        let from = pos;
        let to = pos;
        const k = klass(pos, false);

        while (from > 0 && klass(from - 1, false) === k && cells[from - 1].line === cells[from].line) from--;
        while (to + 1 < cells.length && klass(to + 1, false) === k && cells[to + 1].line === cells[to].line) to++;

        const word = text.slice(from, to + 1);

        pattern = '\\b' + word.replace(/[.*+?^${}()|[\]\\]/g, '\\$&') + '\\b';
        searchDir = dir;
        research();
        jumpTo(dir, pos);
    }

    /* ------------------------------------------------------------------
    * visual mode and yanking
    * ------------------------------------------------------------------ */

    function paintSelection() {
        const selection = window.getSelection();
        if (!selection) return;

        let from = Math.min(anchor, pos);
        let to = Math.max(anchor, pos);

        if (mode === VLINE) {
            from = lineAt(from).first;
            to = lineAt(to).last;
        }

        const r = document.createRange();
        r.setStart(cells[from].node, cells[from].off);
        r.setEnd(cells[to].node, cells[to].off + 1);

        selection.removeAllRanges();
        selection.addRange(r);
    }

    function dropSelection() {
        const selection = window.getSelection();
        if (selection) selection.removeAllRanges();
    }

    function copy(s) {
        const done = () => say(s.length + ' characters yanked', 'mode');

        if (navigator.clipboard && window.isSecureContext) {
            navigator.clipboard.writeText(s).then(done, () => fallbackCopy(s, done));
            return;
        }

        fallbackCopy(s, done);
    }

    function fallbackCopy(s, done) {
        const box = document.createElement('textarea');
        box.value = s;
        box.setAttribute('readonly', '');
        box.style.cssText = 'position:fixed;top:-1000px;opacity:0';

        document.body.append(box);
        box.select();

        let ok = false;
        try { ok = document.execCommand('copy'); } catch (e) { ok = false; }

        box.remove();

        if (ok) done();
        else say('E353: Nothing in register "', 'error');
    }

    function textBetween(from, to) {
        const r = document.createRange();
        r.setStart(cells[from].node, cells[from].off);
        r.setEnd(cells[to].node, cells[to].off + 1);
        return r.toString();
    }

    function yankSelection() {
        let from = Math.min(anchor, pos);
        let to = Math.max(anchor, pos);

        if (mode === VLINE) {
            from = lineAt(from).first;
            to = lineAt(to).last;
        }

        copy(textBetween(from, to));
        leaveVisual();
        setPos(from);
    }

    function leaveVisual() {
        mode = NORMAL;
        dropSelection();
        status();
    }

    /* ------------------------------------------------------------------
    * the command line
    * ------------------------------------------------------------------ */

    let heldPattern = '';

    function openCmd(sign) {
        if (mode === VISUAL || mode === VLINE) leaveVisual();

        mode = CMD;
        restore = pos;
        heldPattern = pattern;

        signEl.textContent = sign;
        cmdEl.value = '';
        cmdBox.hidden = false;
        barEl.classList.add('is-cmd');
        cmdEl.focus();

        status();
    }

    function closeCmd() {
        cmdBox.hidden = true;
        barEl.classList.remove('is-cmd');
        cmdEl.blur();
        mode = NORMAL;
        status();
    }

    /* leaving a search with Esc undoes it: pattern and view both go back */
    function cancelCmd() {
        closeCmd();

        pattern = heldPattern;
        if (pattern) research();
        else dropHighlight();

        setPos(restore);
    }

    function runSearch(src, dir) {
        if (src === '') {
            if (!pattern) return;
        } else {
            pattern = src;
        }

        searchDir = dir;
        research();
        jumpTo(dir, dir > 0 ? restore : restore + 1);
    }

    function excmd(raw) {
        const s = raw.trim();

        if (s === '') return;

        if (/^\d+$/.test(s)) {
            const n = Math.max(1, Math.min(lines.length, parseInt(s, 10)));
            setPos(firstNonBlank(lines[n - 1]));
            return;
        }

        if (s === '$') { setPos(cells.length - 1); return; }

        switch (s) {
            case 'q': case 'q!': case 'qa': case 'qa!':
            case 'quit': case 'x': case 'wq': case 'wq!':
                window.location.href = 'index.html';
                return;

            case 'w': case 'write': case 'w!':
                say("E45: 'readonly' option is set", 'error');
                return;

            case 'h': case 'help': case 'he':
                openHelp();
                return;

            case 'noh': case 'nohl': case 'nohlsearch':
                clearHits();
                say('');
                return;

            case 'set nu': case 'set number': case 'set list': case 'set paste':
                say('E518: Unknown option: ' + s.slice(4), 'error');
                return;

            default:
                say('E492: Not an editor command: ' + s, 'error');
        }
    }

    cmdEl.addEventListener('input', () => {
        /* 'incsearch': the page follows along as the pattern is typed */
        if (signEl.textContent === ':') return;

        const dir = signEl.textContent === '/' ? 1 : -1;

        if (cmdEl.value === '') {
            dropHighlight();
            setPos(restore);
            return;
        }

        pattern = cmdEl.value;
        research();
        jumpTo(dir, dir > 0 ? restore : restore + 1, true);
    });

    cmdEl.addEventListener('keydown', (e) => {
        if (e.key === 'Enter') {
            e.preventDefault();

            const value = cmdEl.value;
            const sign = signEl.textContent;

            closeCmd();

            if (sign === ':') excmd(value);
            else runSearch(value, sign === '/' ? 1 : -1);

            return;
        }

        if (e.key === 'Backspace' && cmdEl.value === '') {
            e.preventDefault();
            closeCmd();
            setPos(restore);
        }
    });

    /* ------------------------------------------------------------------
    * links
    * ------------------------------------------------------------------ */

    function linkAt(i) {
        const el = cells[i].node.parentElement;
        return el ? el.closest('a[href]') : null;
    }

    function follow() {
        const a = linkAt(pos);

        if (!a) {
            say('E446: No file name under cursor', 'error');
            return;
        }

        a.click();
    }

    /* ------------------------------------------------------------------
    * help
    * ------------------------------------------------------------------ */

    const HELP = [
        ['motion', [
            ['h l', 'one character left, right'],
            ['j k', 'one line down, up'],
            ['w b e', 'word forward, back, end'],
            ['W B E', 'the same, whitespace separated'],
            ['0 ^ $', 'line start, first word, line end'],
            ['3|', 'column 3 of this line'],
            ['{ }', 'previous, next block'],
            ['gg G', 'first, last line'],
            ['5G', 'line 5'],
            ['50%', 'halfway down the post'],
            ['H M L', 'top, middle, bottom of the screen'],
            ['f F', 'to a character on this line'],
            ['; ,', 'repeat that, forward or back'],
        ]],
        ['scroll', [
            ['C-d C-u', 'half a screen down, up'],
            ['C-f C-b', 'a whole screen down, up'],
            ['C-e C-y', 'one line down, up'],
            ['zz zt zb', 'this line to the middle, top, bottom'],
        ]],
        ['search', [
            ['/ ?', 'search forward, backward'],
            ['n N', 'next, previous match'],
            ['*', 'search the word under the cursor'],
            [':noh', 'drop the highlighting'],
        ]],
        ['select', [
            ['v V', 'visual, visual line'],
            ['y', 'yank the selection'],
            ['yy Y', 'yank this line'],
            ['o', 'swap the ends of the selection'],
        ]],
        ['elsewhere', [
            ['CR gx', 'open the link under the cursor'],
            ['t', 'light or dark'],
            [':q ZZ', 'back to the index'],
            [':42', 'line 42'],
            ['F1 :help', 'this window'],
            ['Esc', 'never mind'],
        ]],
    ];

    let helpReady = false;

    function fillHelp() {
        const fragment = document.createDocumentFragment();

        for (const [title, rows] of HELP) {
            const section = document.createElement('section');
            const heading = document.createElement('h3');

            heading.textContent = title;
            section.append(heading);

            const list = document.createElement('dl');

            for (const [keys, what] of rows) {
                const dt = document.createElement('dt');

                for (const key of keys.split(' ')) {
                    const kbd = document.createElement('kbd');
                    kbd.textContent = key;
                    dt.append(kbd);
                }

                const dd = document.createElement('dd');
                dd.textContent = what;

                list.append(dt, dd);
            }

            section.append(list);
            fragment.append(section);
        }

        helpBox.append(fragment);
        helpReady = true;
    }

    function openHelp() {
        if (!helpReady) fillHelp();
        helpEl.hidden = false;
    }

    function closeHelp() {
        helpEl.hidden = true;
    }

    helpEl.addEventListener('click', closeHelp);

    /* ------------------------------------------------------------------
    * keys
    * ------------------------------------------------------------------ */

    const NOT_MODIFIABLE = "E21: Cannot make changes, 'modifiable' is off";

    function take(fallback) {
        const n = count === '' ? fallback : parseInt(count, 10);
        count = '';
        return n;
    }

    function repeat(fn) {
        const n = take(1);
        let i = pos;
        for (let k = 0; k < n; k++) i = fn(i);
        setPos(i);
    }

    function onKey(e) {
        if (!cells.length) return;

        /* Esc gets out of the command line, focus and all */
        if (e.key === 'Escape' && mode === CMD) {
            e.preventDefault();
            cancelCmd();
            return;
        }

        if (e.target === cmdEl || e.defaultPrevented) return;

        if (!helpEl.hidden) {
            if (e.metaKey || e.ctrlKey || e.altKey) return;
            e.preventDefault();
            closeHelp();
            return;
        }

        if (e.metaKey || e.altKey) return;

        if (e.ctrlKey) {
            const page = window.innerHeight - barTop() - barBottom();
            const line = cells[pos].h * 1.55;

            switch (e.key.toLowerCase()) {
                case 'd': scrollPage(page / 2); break;
                case 'u': scrollPage(-page / 2); break;
                case 'f': scrollPage(page); break;
                case 'b': scrollPage(-page); break;
                case 'e': scrollView(line); break;
                case 'y': scrollView(-line); break;
                default: return;
            }

            e.preventDefault();
            status();
            return;
        }

        const key = e.key;

        /* f and F hold the next keystroke hostage */
        if (pending === 'f' || pending === 'F') {
            const dir = pending === 'f' ? 1 : -1;
            const n = take(1);

            pending = '';
            e.preventDefault();

            if (key.length === 1) {
                lastFind = { ch: key, dir: dir };
                findChar(key, dir, n);
            }

            status();
            return;
        }

        if (key.length === 1 && key >= '0' && key <= '9' && !(key === '0' && count === '')) {
            count += key;
            e.preventDefault();
            status();
            return;
        }

        if (pending) {
            const held = pending;
            pending = '';
            e.preventDefault();
            afterPending(held, key);
            return;
        }

        switch (key) {
            /* --- character and line --- */
            case 'h': case 'ArrowLeft':
                repeat((i) => Math.max(lineAt(i).first, i - 1));
                break;

            case 'l': case 'ArrowRight': case ' ':
                repeat((i) => Math.min(lineAt(i).last, i + 1));
                break;

            case 'j': case 'ArrowDown': vertical(take(1)); break;
            case 'k': case 'ArrowUp': vertical(-take(1)); break;

            case '0': setPos(lineAt(pos).first); break;
            case '^': setPos(firstNonBlank(lineAt(pos))); break;

            case '$': {
                const line = Math.min(lines.length - 1, cells[pos].line + take(1) - 1);
                wantX = Infinity;
                setPos(lines[line].last, true);
                break;
            }

            case '|': {
                const line = lineAt(pos);
                setPos(Math.min(line.last, line.first + take(1) - 1));
                break;
            }

            /* --- words and blocks --- */
            case 'w': repeat((i) => wordForward(i, false)); break;
            case 'W': repeat((i) => wordForward(i, true)); break;
            case 'b': repeat((i) => wordBack(i, false)); break;
            case 'B': repeat((i) => wordBack(i, true)); break;
            case 'e': repeat((i) => wordEnd(i, false)); break;
            case 'E': repeat((i) => wordEnd(i, true)); break;
            case '}': repeat(blockForward); break;
            case '{': repeat(blockBack); break;

            /* --- whole buffer --- */
            case 'G': {
                const n = take(0);
                setPos(n
                    ? firstNonBlank(lines[Math.min(lines.length, n) - 1])
                    : firstNonBlank(lines[lines.length - 1]));
                break;
            }

            case '%': {
                const n = take(0);

                if (!n) {
                    const at = matchBracket(pos);
                    if (at >= 0) setPos(at);
                    break;
                }

                const line = Math.min(lines.length - 1, Math.floor((Math.min(n, 100) / 100) * lines.length));
                setPos(firstNonBlank(lines[line]));
                break;
            }

            /* --- screen --- */
            case 'H': case 'M': case 'L':
                setPos(firstNonBlank(lines[screenLine(key)]));
                break;

            /* --- pending prefixes --- */
            case 'y':
                if (mode === VISUAL || mode === VLINE) {
                    yankSelection();
                    break;
                }
                /* falls through: in normal mode y waits for the second y */

            case 'g': case 'z': case 'Z': case 'f': case 'F':
                pending = key;
                e.preventDefault();
                status();
                return;

            case ';': case ',': {
                if (!lastFind) break;
                const dir = key === ';' ? lastFind.dir : -lastFind.dir;
                findChar(lastFind.ch, dir, take(1));
                break;
            }

            /* --- search --- */
            case '/': openCmd('/'); break;
            case '?': openCmd('?'); break;
            case ':': openCmd(':'); break;

            case 'n': jumpTo(searchDir, pos); break;
            case 'N': jumpTo(-searchDir, pos); break;
            case '*': searchWordUnderCursor(1); break;
            case '#': searchWordUnderCursor(-1); break;

            /* --- visual --- */
            case 'v':
                if (mode === VISUAL) leaveVisual();
                else {
                    if (mode !== VLINE) anchor = pos;
                    mode = VISUAL;
                    paintSelection();
                    status();
                }
                break;

            case 'V':
                if (mode === VLINE) leaveVisual();
                else {
                    if (mode !== VISUAL) anchor = pos;
                    mode = VLINE;
                    paintSelection();
                    status();
                }
                break;

            case 'Y': {
                const line = lineAt(pos);
                copy(textBetween(line.first, line.last));
                break;
            }

            /* --- links --- */
            case 'Enter': follow(); break;

            case 'o':
                if (mode === VISUAL || mode === VLINE) {
                    const other = anchor;
                    anchor = pos;
                    setPos(other);
                } else {
                    follow();
                }
                break;

            /* --- help --- */
            case 'F1': openHelp(); break;

            case 'Escape':
                if (mode === VISUAL || mode === VLINE) leaveVisual();
                count = '';
                pending = '';
                say('');
                break;

            /* --- there is nothing to edit here --- */
            case 'i': case 'I': case 'a': case 'A': case 'x': case 'X':
            case 'd': case 'D': case 'c': case 'C': case 's': case 'S':
            case 'r': case 'R': case 'p': case 'P': case 'J': case 'u':
                say(NOT_MODIFIABLE, 'error');
                break;

            default:
                return;
        }

        e.preventDefault();
        hinted = true;
        status();
    }

    function afterPending(held, key) {
        if (held === 'g') {
            switch (key) {
                case 'g': {
                    const n = take(0);
                    setPos(n ? firstNonBlank(lines[Math.min(lines.length, n) - 1]) : 0);
                    break;
                }
                case 'j': vertical(take(1)); break;
                case 'k': vertical(-take(1)); break;
                case 'x': follow(); break;
                case 'e': repeat((i) => wordEndBack(i, false)); break;
                case 'E': repeat((i) => wordEndBack(i, true)); break;
                case '_': setPos(firstNonBlank(lineAt(pos))); break;
                case '?': openHelp(); break;
                default: count = '';
            }
        } else if (held === 'z') {
            if (key === 'z' || key === 't' || key === 'b') redraw(key);
            count = '';
        } else if (held === 'y') {
            if (key === 'y') {
                const line = lineAt(pos);
                copy(textBetween(line.first, line.last));
            }
            count = '';
        } else if (held === 'Z') {
            if (key === 'Z' || key === 'Q') window.location.href = 'index.html';
            count = '';
        }

        status();
    }

    /*
    * capture, so that a key this layer claims is marked handled before
    * the theme toggle in the page head looks at it: zt and ft are ours,
    * a bare t is still the theme.
    * */
    document.addEventListener('keydown', onKey, true);

    /* ------------------------------------------------------------------
    * mouse, resize, boot
    * ------------------------------------------------------------------ */

    function cellAtPoint(x, y) {
        let best = -1;
        let dist = Infinity;

        for (let i = 0; i < lines.length; i++) {
            const line = lines[i];
            const dy = y < line.top ? line.top - y : y > line.bottom ? y - line.bottom : 0;

            if (dy > dist) continue;

            for (let j = line.first; j <= line.last; j++) {
                const c = cells[j];
                const dx = x < c.x ? c.x - x : x > c.x + c.w ? x - (c.x + c.w) : 0;
                const d = dy * 4 + dx;

                if (d < dist) { dist = d; best = j; }
            }
        }

        return best;
    }

    document.addEventListener('mouseup', (e) => {
        if (mode === CMD || !cells.length) return;
        if (!root.contains(e.target)) return;

        const selection = window.getSelection();
        if (selection && !selection.isCollapsed) return;

        const i = cellAtPoint(e.clientX + window.scrollX, e.clientY + window.scrollY);
        if (i >= 0) {
            if (mode === VISUAL || mode === VLINE) leaveVisual();
            setPos(i);
        }
    });

    let remeasure = 0;

    function schedule() {
        clearTimeout(remeasure);
        remeasure = setTimeout(() => { build(); draw(); }, 120);
    }

    window.addEventListener('resize', schedule);
    window.addEventListener('load', schedule);

    /* a code block scrolled sideways invalidates every box it holds */
    document.addEventListener('scroll', (e) => {
        if (e.target !== document) schedule();
    }, true);

    if (window.ResizeObserver) new ResizeObserver(schedule).observe(root);

    build();

    if (!cells.length) return;

    fileEl.textContent = '"' + (location.pathname.split('/').pop() || 'index.html') + '" [readonly]';
    barEl.hidden = false;
    draw();

    if (document.fonts && document.fonts.ready) document.fonts.ready.then(schedule);
})();
