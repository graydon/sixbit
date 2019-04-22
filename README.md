# sixbit

## Sixbit - a crate for small packed strings.

This crate provides densely-packed 8-bit, 16-bit, 32-bit, 64-bit, and
128-bit "small strings", in a variety of custom script-specific
6-bit-per-character encodings. It does not deal with any scripts that have
repertoires that significantly exceed reasonable 6-bit-per-character codes
(eg. Chinese); such scripts are probably better off using UTF-16 or BOCU-1.

(Also its treatment of Latin leaves something to be desired, since you
cannot encode mixed-case strings; this is a deliberate choice to let us do
alphanumeric-with-symbols strings in at least one case -- upper case.)

This sort of datatype is a low-level optimization to use in a system
processing a lot of small strings (eg. single words): you can avoid
allocating them on the heap, store them in string-structure headers, NaN
boxes or other sorts of small-literal value types.

Perhaps most enticingly: you can pack _more than one of them_ into a SIMD
vector, and operate on (eg. search) multiple such strings at once, in
parallel, as a single register. Vector registers just keep growing these
days.

Of course not every string is short enough to fit, and not every
short-enough string has codepoints that fall inside one of the "code pages"
that this crate provides. The encoding functions are therefore all
partial. But they should handle a significant enough quantity of strings to
make it worthwhile.

### Code Pages

Every packed string produced by this crate begins with a small tag
indicating the "code page" of the rest of the string. A code page here is a
set of 64 unicode character values that the 6-bit codes of the rest of the
string are interpreted as. Strings that mix characters from multiple code
pages are not supported. Again, think "single words".

I have chosen the contents of the code pages to the best of my abilities in
script knowledge, guesswork, consulting with friends and experts, scouring
wikipedia and so forth, and subject to some constraints outlined below. I'm
happy to take PRs to improve the contents of them to better capture "many
words" in specific scripts; code page contents will therefore be slightly in
flux until we get to a 1.0 release, so if by some bizarre chance you are
reading this and choose to use the crate and are going to store values from
this crate in stable storage, you should lock your client to a specific
point-revision of the crate until 1.0.

#### Constraints

There is only enough room in the tag to reference a handful of code pages;
not every script will make it, but luckily only a few scripts account for
much of the text in the world.

We want to avoid wasting bits, and the number of spare bits in a packed
value of N bits (modulo 6) varies, depending on its size: 2 spare bits for
8, 32 and 128-bit values; 4 spare bits for 16 and 64-bit values.

We want to be able to sort these strings using machine-provided integer
comparison, and have that order correspond to unicode code-point
lexicographical string order (including "short strings sort before
long"). This dictates a fair amount about the tag values, code repertoires,
and normal form of the encoded strings.

#### Design

Code pages are taken from (or in some cases, across) unicode blocks, and
tags are ordered by (initial) unicode block. Codes within each code page are
further ordered by unicode codepoint: each code page is essentially a
"likely-useful subsequence" of the contents of 1 or more corresponding
unicode block(s). This unfortunately means that common punctuation
characters and digits are only available for strings using the
uppercase-latin page. I've tried to include some additional punctuation
where it's available in blocks. Since mixing pages is also not possible,
"supplementary" blocks have been mostly avoided unless they happen to follow
an unused sequence-prefix that can be combined to make a useful
self-contained page (eg. the second "lower case and extended latin" page).

A script can only work if there's a "likely-useful subsequence" that fits
inside 63 code points. The zero code in each page is reserved as the string
_terminator_ and padding value. Strings that encode shorter than their
packed value container are left-shifted to always begin at the
most-significant coding bit, and the trailing bits are all set to zero (this
does not mean you can encode nulls -- the zeroes here mean "past end of
string").

The high order / 2-bit tags select among 4 "primary" (most-likely) scripts
spread across the unicode range (in code block order). These are available
in all packed values:

  | tag | page contents                              |
  |-----|--------------------------------------------|
  |  00 | Latin upper case, digits and punctuation   |
  |  01 | Cyrillic                                   |
  |  10 | Arabic                                     |
  |  11 | Devanagari                                 |

When packing 64-bit and 16-bit values we get _4_ spare bits to use for a
tag, not just 2. In these cases we therefore have 12 additional scripts
available, which for simplicity sake casting up and down between value sizes
we keep the high order bits the same and add 2 bits below, picking
additional scripts _from the block ranges between_ those of the primary
scripts (again, in unicode block order):

  | tag   | page contents                                 |
  |-------|-----------------------------------------------|
  | 00 00 | Latin upper case, digits and punctuation      |
  | 00 01 | Latin lower case and extended lowercase forms |
  | 00 10 | *reserved*                                    |
  | 00 11 | Greek                                         |
  |       |                                               |
  | 01 00 | Cyrillic                                      |
  | 01 01 | *reserved*                                    |
  | 01 10 | Hebrew                                        |
  | 01 11 | *reserved*                                    |
  |       |                                               |
  | 10 00 | Arabic                                        |
  | 10 01 | *reserved*                                    |
  | 10 10 | *reserved*                                    |
  | 10 11 | *reserved*                                    |
  |       |                                               |
  | 11 00 | Devanagari                                    |
  | 11 01 | *reserved*                                    |
  | 11 10 | Hangul Compatibility Jamo                     |
  | 11 11 | Halfwidth Kana                                |

The *reserved* cases are where I either didn't know enough about the scripts
available in that range of unicode, or ran out of good candidates, or both.
I might assign them to something in the future, or "compact out" the gaps /
reassign the 4-bit codes so their high bits are _not_ the same as the 2-bit
cases, but I've already exceeded my realistic level of armchair-linguist
knowledge and I figured simplifying design choices would be better than
pretending I could do any better. Patches welcome!

The overall assignment of bits is summarized as follows:

| packed type | tag bits | coding bits | max string length  |
|-------------|----------|-------------|--------------------|
| u128        | 2        | 126         | 21                 |
|  u64        | 4        |  60         | 10                 |
|  u32        | 2        |  30         |  5                 |
|  u16        | 4        |  12         |  2                 |
|   u8        | 2        |   6         |  1                 |


License: MIT OR Apache-2.0
