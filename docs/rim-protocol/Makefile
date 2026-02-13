MAIN = rim-self-data-flows
LATEX = xelatex
LATEX_FLAGS = -interaction=nonstopmode -halt-on-error

.PHONY: all clean

all: $(MAIN).pdf

$(MAIN).pdf: $(MAIN).tex
	$(LATEX) $(LATEX_FLAGS) $(MAIN).tex
	$(LATEX) $(LATEX_FLAGS) $(MAIN).tex  # second pass for TOC/refs

clean:
	rm -f $(MAIN).pdf $(MAIN).aux $(MAIN).log $(MAIN).toc $(MAIN).out
