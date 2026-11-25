# A Clean session holds the file twice when steps are on

Each open Clean session keeps the table as read, plus the cleaned table when at least one step is enabled. With no
enabled steps the result shares the first table, so there is one copy. A second open of the same file is a second
session with its own copies. At most three sessions can be open. Reading rows back from disk on demand would halve
this, at the cost of speed.
