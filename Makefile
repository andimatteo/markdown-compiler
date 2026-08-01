SHELL := /bin/bash
.DEFAULT_GOAL := help

POSTS_DIR := posts
MEDIA_DIR := static

GIF_FPS := 18
GIF_MIN_FPS := 6
GIF_MAX_WIDTH := 720
GIF_MIN_WIDTH := 240
GIF_COLORS := 128
GIF_TARGET_KB := 4096

LINK_PATH := $(word 2,$(MAKECMDGOALS))

.PHONY: help post gifs link

ifeq ($(firstword $(MAKECMDGOALS)),link)
ifneq ($(LINK_PATH),)
.PHONY: $(LINK_PATH)
$(LINK_PATH):
	@:
endif
endif

help:
	@printf '%s\n' \
		'make post                  Create a new post template' \
		'make gifs                  Convert .mov/.mp4 files in static/ to ~1 MB GIFs' \
		'make link /path/to/folder  Link a directory inside posts/'

post:
	@mkdir -p "$(POSTS_DIR)"
	@read -r -p "Title: " title; \
	read -r -p "Date [$(shell date +%F)]: " date; \
	read -r -p "Description: " description; \
	read -r -p "Comma-separated tags: " tags; \
	if [[ -z "$$title" ]]; then \
		printf 'The title cannot be empty.\n' >&2; \
		exit 1; \
	fi; \
	if [[ -z "$$date" ]]; then \
		date="$(shell date +%F)"; \
	fi; \
	slug="$$(printf '%s' "$$title" \
		| tr '[:upper:]' '[:lower:]' \
		| sed -E 's/[^a-z0-9]+/-/g; s/^-+//; s/-+$$//')"; \
	file="$(POSTS_DIR)/$$slug.md"; \
	if [[ -e "$$file" ]]; then \
		printf 'The file %s already exists.\n' "$$file" >&2; \
		exit 1; \
	fi; \
	printf '%s\n' \
		'---' \
		"title: $$title" \
		"date: $$date" \
		"description: $$description" \
		"tags: [$$tags]" \
		'---' \
		'' \
		> "$$file"; \
	printf 'Created %s\n' "$$file"

gifs:
	@command -v ffmpeg >/dev/null || { \
		printf 'ffmpeg was not found. Install it with: brew install ffmpeg\n' >&2; \
		exit 1; \
	}
	@set -euo pipefail; \
	found=0; \
	target_bytes=$$(( $(GIF_TARGET_KB) * 1024 )); \
	while IFS= read -r -d '' input; do \
		found=1; \
		output="$${input%.*}.gif"; \
		width=$(GIF_MAX_WIDTH); \
		fps=$(GIF_FPS); \
		colors=$(GIF_COLORS); \
		printf 'Converting %s -> %s\n' "$$input" "$$output"; \
		while :; do \
			ffmpeg \
				-nostdin \
				-hide_banner \
				-loglevel warning \
				-stats \
				-y \
				-i "$$input" \
				-filter_complex "[0:v]fps=$$fps,scale='min($$width,iw)':-2:flags=lanczos,split[a][b];[a]palettegen=max_colors=$$colors:stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=4:diff_mode=rectangle" \
				-loop 0 \
				"$$output"; \
			size=$$(stat -f '%z' "$$output" 2>/dev/null || stat -c '%s' "$$output"); \
			printf 'Generated %d KB — width=%d, fps=%d, colors=%d\n' \
				"$$((size / 1024))" "$$width" "$$fps" "$$colors"; \
			if (( size <= target_bytes )); then \
				break; \
			elif (( width > $(GIF_MIN_WIDTH) )); then \
				width=$$((width * 85 / 100)); \
				if (( width < $(GIF_MIN_WIDTH) )); then \
					width=$(GIF_MIN_WIDTH); \
				fi; \
			elif (( colors > 32 )); then \
				colors=$$((colors / 2)); \
			elif (( fps > $(GIF_MIN_FPS) )); then \
				fps=$$((fps - 2)); \
				if (( fps < $(GIF_MIN_FPS) )); then \
					fps=$(GIF_MIN_FPS); \
				fi; \
			else \
				printf 'Warning: could not reduce %s below %d KB.\n' \
					"$$output" "$(GIF_TARGET_KB)" >&2; \
				break; \
			fi; \
		done; \
	done < <(find "$(MEDIA_DIR)" -type f \
		\( -iname '*.mov' -o -iname '*.mp4' \) -print0); \
	if [[ $$found -eq 0 ]]; then \
		printf 'No MOV or MP4 files found in %s\n' "$(MEDIA_DIR)"; \
	fi

link:
	@set -euo pipefail; \
	source="$(LINK_PATH)"; \
	if [[ -z "$$source" ]]; then \
		printf 'Usage: make link /path/to/directory\n' >&2; \
		exit 1; \
	fi; \
	if [[ ! -d "$$source" ]]; then \
		printf 'Directory not found: %s\n' "$$source" >&2; \
		exit 1; \
	fi; \
	mkdir -p "$(POSTS_DIR)"; \
	source="$$(cd "$$source" && pwd -P)"; \
	destination="$(POSTS_DIR)/$$(basename "$$source")"; \
	if [[ -e "$$destination" || -L "$$destination" ]]; then \
		printf 'Destination already exists: %s\n' "$$destination" >&2; \
		exit 1; \
	fi; \
	ln -s "$$source" "$$destination"; \
	printf 'Created %s -> %s\n' "$$destination" "$$source"
