# Clean preview looks at the window only

`kh_clean::preview` runs the earlier steps and the previewed step on the visible rows only, as spec 5.2 says.
"Remove duplicate rows" therefore marks a row as a duplicate only when its twin is also in the window. The full
run (`replay`) is exact. Also left out: spelling merge only matches the exact spellings the person ticked, so a new
spelling in next week's file is left as it is and is not reported.
