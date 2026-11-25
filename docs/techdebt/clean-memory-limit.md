# A hostile CSV can use far more memory than the budget

One table may hold 1 GiB of cell text and three Clean sessions may be open, each with a second cleaned copy.
Lower the table limit to fit the memory budget in spec section 4 and count open sessions by size, not number.
