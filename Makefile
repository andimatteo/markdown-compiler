SHELL := /bin/bash
.DEFAULT_GOAL := help

POSTS_DIR := posts
MEDIA_DIR := static
GIF_FPS := 18
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
		'make post    Create new post template' \
		'make gifs    Convert .mp4 to .gif in static/'

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
	while IFS= read -r -d '' input; do \
		found=1; \
		output="$${input%.*}.gif"; \
		printf 'Converting %s -> %s\n' "$$input" "$$output"; \
		ffmpeg \
			-nostdin \
			-hide_banner \
			-loglevel warning \
			-stats \
			-y \
			-i "$$input" \
			-filter_complex "[0:v]fps=$(GIF_FPS),setpts=N/($(GIF_FPS)*TB),split[a][b];[a]palettegen=max_colors=256:stats_mode=diff[p];[b][p]paletteuse=dither=sierra2_4a:diff_mode=rectangle" \
			-loop 0 \
			"$$output"; \
	done < <(find "$(MEDIA_DIR)" -type f -iname '*.mp4' -print0); \
	if [[ $$found -eq 0 ]]; then \
		printf 'No MP4 files found in %s\n' "$(MEDIA_DIR)"; \
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
