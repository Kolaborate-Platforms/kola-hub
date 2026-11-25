# Typed cell fixes are lost when the steps change

`clean_fix_cell` changes one cell of the result held in the session. The fix is kept by row and column of that
result, and `kh-clean` gives no stable row id to follow, so any change to the step list (add, undo, move, switch,
remove) drops the fixes. A change that fails and is rolled back keeps them. Saving writes them into the new file.
A better design is a "set cell" step keyed by source row, which needs a change in `kh-clean`.
