// -*- mode: rust; bidi-display-reordering: nil -*-

//! # Sixbit - a crate for small packed strings.
//!
//! This crate provides densely-packed 8-bit, 16-bit, 32-bit, 64-bit, and
//! 128-bit "small strings", in a variety of custom script-specific
//! 6-bit-per-character encodings, as well as a special 15-bit-per-character
//! encoding for the set of Chinese characters in the "Unified Repertoire and
//! Ordering" (URO) block of Unicode.
//!
//! This sort of datatype is a low-level optimization to use in a system
//! processing a lot of small strings (eg. single words): you can avoid
//! allocating them on the heap, store them in string-structure headers, NaN
//! boxes or other sorts of small-literal value types.
//!
//! Perhaps most enticingly: you can pack _more than one of them_ into a SIMD
//! vector, and operate on (eg. search) multiple such strings at once, in
//! parallel, as a single register. Vector registers just keep growing these
//! days.
//!
//! Of course not every string is short enough to fit, and not every
//! short-enough string has codepoints that fall inside one of the "code pages"
//! that this crate provides. The encoding functions are therefore all partial.
//! But they should handle a significant enough quantity of strings to make it
//! worthwhile.
//!
//! ## Usage Summary
//!
//! Encoding is done via the `EncodeSixbit` trait attached to `Iterator<char>`,
//! so you can just do: `let enc = "hello".chars().encode_sixbit::<u64>()`. If
//! there is a failure (say, the string spans pages or doesn't fit) you'll get
//! back an `Err(EncodeError)` with details, otherwise `Ok(n)` where `n` is a
//! `u64`.
//!
//! Decoding is a `DecodeSixbitIter` iterator implementing `Iterator<char>`,
//! attached to the various packed types, allowing you to write `let s:String =
//! someu64.decode_sixbit().collect()`, or any other pattern that takes an
//! `Iterator<char>`.
//!
//! In several cases you will need to normalize or decompose "standard" unicode
//! text before pushing it through these interfaces. For example, the Hangul
//! page only has compatibility jamo, so you have to decompose standard Korean
//! text to that form before encoding. Similarly the Halfwidth Kana are unlikely
//! to be the characters standard Japanese text arrives in, and Devanagari
//! strings with nuktas will need to be decomposed before mapping. This crate
//! does none of these tasks: it's a building block, not a complete solution.
//!
//! ## Code Pages
//!
//! Every packed string produced by this crate begins with a small tag
//! indicating the "code page" of the rest of the string. A code page here is a
//! set of 64 unicode character values that the 6-bit codes of the rest of the
//! string are interpreted as (or, as a special case, the URO block). Strings
//! that mix characters from multiple code pages are not supported. Again, think
//! "single words".
//!
//! I have chosen the contents of the code pages to the best of my abilities in
//! script knowledge, guesswork, consulting with friends and experts, scouring
//! wikipedia and so forth, and subject to some constraints outlined below. I'm
//! happy to take PRs to improve the contents of them to better capture "many
//! words" in specific scripts; code page contents will therefore be slightly in
//! flux until we get to a 1.0 release, so if by some bizarre chance you are
//! reading this and choose to use the crate and are going to store values from
//! this crate in stable storage, you should lock your client to a specific
//! point-revision of the crate until 1.0.
//!
//! ### Constraints
//!
//! There is only enough room in the tag to reference a handful of code pages;
//! not every script will make it, but luckily only a few scripts account for
//! much of the text in the world.
//!
//! We want to avoid wasting bits, and the number of spare bits in a packed
//! value of N bits (modulo 6) varies, depending on its size: 2 spare bits for
//! 8, 32 and 128-bit values; 4 spare bits for 16 and 64-bit values.
//!
//! The tag for Chinese is allocated in all cases but there's only space for
//! a nonzero sequence of the 15-bit codes in 32, 64 and 128-bit values, so
//! only those widths use it; in 8 or 16-bit cases the tag is just reserved.
//!
//! We want to be able to sort these strings using machine-provided integer
//! comparison, and have that order correspond to unicode code-point
//! lexicographical string order (including "short strings sort before long").
//! This dictates a fair amount about the tag values, code repertoires, and
//! normal form of the encoded strings.
//!
//! ### Design
//!
//! Code pages are taken from (or in some cases, across) unicode blocks, and
//! tags are ordered by (initial) unicode block. Codes within each code page are
//! further ordered by unicode codepoint: each code page is essentially a
//! "likely-useful subsequence" of the contents of 1 or more corresponding
//! unicode block(s). This unfortunately means that digits are only available in
//! strings using the latin page (which also has underscore -- there is one
//! space extra and it's common in many identifier repertoires). I've tried to
//! include some additional punctuation where it's available in blocks. Since
//! mixing pages is also not possible, "supplementary" blocks have been mostly
//! avoided.
//!
//! A script can only work if there's a "likely-useful subsequence" that fits
//! inside 63 code points (unless it's the URO block). The zero code in each
//! page is reserved as the string _terminator_ and padding value. Strings that
//! encode shorter than their packed value container are left-shifted to always
//! begin at the most-significant coding bit, and the trailing bits are all set
//! to zero (this does not mean you can encode nulls -- the zeroes here mean
//! "past end of string").
//!
//! The high order / 2-bit tags select among 4 "primary" (most-likely) scripts
//! spread across the unicode range (in code block order). These are available
//! in all packed values:
//!
//!   | tag | page contents                              |
//!   |-----|--------------------------------------------|
//!   |  00 | Latin (with digits and underscore)         |
//!   |  01 | Arabic                                     |
//!   |  10 | Devanagari                                 |
//!   |  11 | Chinese                                    |
//!
//! When packing 64-bit and 16-bit values we get _4_ spare bits to use for a
//! tag, not just 2. In these cases we therefore have 12 additional scripts
//! available, which for simplicity sake casting up and down between value sizes
//! we keep the high order bits the same and add 2 bits below, picking
//! additional scripts _from the block ranges between_ those of the primary
//! scripts (again, in unicode block order):
//!
//!   | tag   | page contents                                 |
//!   |-------|-----------------------------------------------|
//!   | 00 00 | Latin (with digits and underscore)            |
//!   | 00 01 | Greek                                         |
//!   | 00 10 | Cyrillic                                      |
//!   | 00 11 | Hebrew                                        |
//!   |       |                                               |
//!   | 01 00 | Arabic                                        |
//!   | 01 01 | *reserved*                                    |
//!   | 01 10 | *reserved*                                    |
//!   | 01 11 | *reserved*                                    |
//!   |       |                                               |
//!   | 10 00 | Devanagari                                    |
//!   | 10 01 | *reserved*                                    |
//!   | 10 10 | *reserved*                                    |
//!   | 10 11 | Hangul Compatibility Jamo                     |
//!   |       |                                               |
//!   | 11 00 | Chinese                                       |
//!   | 11 01 | *reserved*                                    |
//!   | 11 10 | *reserved*                                    |
//!   | 11 11 | Halfwidth Kana                                |
//!
//! The *reserved* cases are where I either didn't know enough about the scripts
//! available in that range of unicode, or ran out of good candidates, or both.
//! I might assign them to something in the future, or "compact out" the gaps /
//! reassign the 4-bit codes so their high bits are _not_ the same as the 2-bit
//! cases, but I've already exceeded my realistic level of armchair-linguist
//! knowledge and I figured simplifying design choices would be better than
//! pretending I could do any better. Patches welcome!
//!
//! The overall assignment of bits is summarized as follows:
//!
//! | packed type | tag bits | coding bits | max 6-bit chars | max 15-bit chars |
//! |-------------|----------|-------------|-----------------|------------------|
//! | u128        | 2        | 126         | 21              | 8                |
//! |  u64        | 4        |  60         | 10              | 4                |
//! |  u32        | 2        |  30         |  5              | 2                |
//! |  u16        | 4        |  12         |  2              | 0                |
//! |   u8        | 2        |   6         |  1              | 0                |

use std::ops::{BitOrAssign, ShlAssign};
use std::mem::size_of;

// Page 00 00: U+0000, then U+0030-U+0039, U+0041-U+005A, U+005F, and U+0061-U+007A.
// Enough to encode the common [a-zA-Z0-9_] character class used in many programming
// language and data format identifier repertoires.
const LATIN : [char; 64] = [
    '\0', '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'A', 'B', 'C', 'D', 'E',
    'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M', 'N', 'O', 'P', 'Q', 'R', 'S', 'T', 'U',
    'V', 'W', 'X', 'Y', 'Z', '_', 'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j',
    'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z'
];

// Page 00 11: U+0000, then upper and lowercase characters in order from
// U+0386-U+03CE, including stressed forms but omitting diaeresis forms.
const GREEK : [char; 64] = [
    '\0',
    // 7 stressed uppercase characters
    'Ά', 'Έ', 'Ή', 'Ί', 'Ό', 'Ύ', 'Ώ',
    // 24 uppercase characters
    'Α', 'Β', 'Γ', 'Δ', 'Ε', 'Ζ', 'Η', 'Θ', 'Ι', 'Κ', 'Λ', 'Μ', 'Ν', 'Ξ', 'Ο', 'Π',
    'Ρ', 'Σ', 'Τ', 'Υ', 'Φ', 'Χ', 'Ψ', 'Ω',
    // 4 stressed lowercase characters
    'ά', 'έ', 'ή', 'ί',
    // 25 lowercase characters (two variants of sigma)
    'α', 'β', 'γ', 'δ', 'ε', 'ζ', 'η', 'θ', 'ι', 'κ', 'λ', 'μ', 'ν', 'ξ', 'ο', 'π',
    'ρ', 'ς', 'σ', 'τ', 'υ', 'φ', 'χ', 'ψ', 'ω',
    // 3 stressed lowercase characters
    'ό', 'ύ', 'ώ'
];

// Page 01 00: U+0000, then U+0410-U+044F excepting the lowercase hard-sign U+044A
const CYRILLIC : [char; 64] = [
    '\0', 'А', 'Б', 'В', 'Г', 'Д', 'Е', 'Ж', 'З', 'И', 'Й', 'К', 'Л', 'М', 'Н', 'О',
    'П', 'Р', 'С', 'Т', 'У', 'Ф', 'Х', 'Ц', 'Ч', 'Ш', 'Щ', 'Ъ', 'Ы', 'Ь', 'Э', 'Ю',
    'Я', 'а', 'б', 'в', 'г', 'д', 'е', 'ж', 'з', 'и', 'й', 'к', 'л', 'м', 'н', 'о',
    'п', 'р', 'с', 'т', 'у', 'ф', 'х', 'ц', 'ч', 'ш', 'щ', 'ы', 'ь', 'э', 'ю', 'я'
];

// Page 01 01: U+0000, then U+05B0-U+05F4
const HEBREW : [char; 64] = [
    '\0', 'ְ', 'ֱ', 'ֲ', 'ֳ', 'ִ', 'ֵ', 'ֶ', 'ַ', 'ָ', 'ֹ', 'ֺ', 'ֻ', 'ּ', 'ֽ', '־',
    'ֿ', '׀', 'ׁ', 'ׂ', '׃', 'ׄ', 'ׅ', '׆', 'ׇ', 'א', 'ב', 'ג', 'ד', 'ה', 'ו', 'ז',
    'ח', 'ט', 'י', 'ך', 'כ', 'ל', 'ם', 'מ', 'ן', 'נ', 'ס', 'ע', 'ף', 'פ', 'ץ', 'צ',
    'ק', 'ר', 'ש', 'ת', 'װ', 'ױ', 'ײ', '׳', '״',
    // Space for 7 more, not sure which to include: expert help wanted!
    '\u{ffff}', '\u{ffff}', '\u{ffff}', '\u{ffff}',
    '\u{ffff}', '\u{ffff}', '\u{ffff}',
];

// Page 10 00: U+0000, then a selection (leaning Perso-Arabic) from the Arabic
// block U+0600–U+06FF, detailed below. Characters selected on the advice of
// @Manishearth who, unlike me, knows something about Arabic script.
const ARABIC : [char; 64] = [
    '\0',

    // 3 punctuators
    // U+060C comma
    '،',
    // U+061B semicolon
    '؛',
    // U+061F question mark
    '؟',

    // 1 hamza
    'ء',

    // 29 main characters in range U+0627-U+0649
    'ا', 'ب', 'ة', 'ت', 'ث', 'ج', 'ح', 'خ',
    'د', 'ذ', 'ر', 'ز', 'س', 'ش', 'ص', 'ض',
    'ط', 'ظ', 'ع', 'غ',
    // omit: U+063B-U+063F "early Persian and Azerbaijani"
    // omit: U+0640 kashida
    'ف', 'ق',
    // omit: U+0643 isolated kaf
    'ل', 'م', 'ن', 'ه', 'و', 'ى', 'ي',

    // 3 short vowels and 1 shadda
    // (sorry my editor balked at displaying some literals here)
    // fatha     damma       kasra       shadda
    '\u{064e}', '\u{064f}', '\u{0650}', '\u{0651}',
    // 2 combining forms of maddah and hamza
    // maddah    hamza
    '\u{0653}', '\u{0654}',
    // 2 vowels used only in Urdu
    // subscript alef
    '\u{0656}', 
    // inverted damma / ulta pesh
    '\u{0657}',

    // 1 superscript alef
    '\u{0670}',

    // 11 extended characters for Persian or Urdu
    // U+0679 tteh (Urdu)
    'ٹ',
    // U+067E peh (Persian, Urdu)
    'پ',
    // U+0686 tcheh (Persian, Urdu)
    'چ',
    // U+0688 ddal (Urdu)
    'ڈ',
    // U+0691 rreh (Urdu)
    'ڑ',
    // U+0698 jeh (Persian, Urdu)
    'ژ',
    // U+06A9 keheh / kaf (Persian, Urdu)
    'ک',
    // U+06AF gaf (Persian, Urdu)
    'گ',
    // U+06BA noon ghunna (Urdu)
    'ں',
    // U+06BE heh doachashmee (Urdu)
    'ھ',
    // U+06D2 yeh barree (Urdu)
    'ے',

    // U+06D4 full stop
    '۔',

    // Space for 9 more, not sure which to include; expert help wanted!
    '\u{ffff}', '\u{ffff}', '\u{ffff}', '\u{ffff}',
    '\u{ffff}', '\u{ffff}', '\u{ffff}', '\u{ffff}',
    '\u{ffff}',
];

// Page 11 00: U+0000, then a selection detailed below from U+0901-U+0965;
// characters selected on the advice of @Manishearth who, unlike me, knows
// something about Devanagari script.
const DEVANAGARI : [char; 64] = [
    '\0',
    // 3 diacritics U+901 candrabindu, U+902 anusvara, U+903 visarga
    'ँ', 'ं', 'ः',
    // omit: U+0904 short A (Awadhi)
    // 11 standalone vowels (U+0905-U+0914)
    'अ', 'आ', 'इ', 'ई', 'उ', 'ऊ', 'ऋ',
    // omit: U+090C vocalic L (Sanskrit)
    // omit: U+090D candra E (transcription)
    // omit: U+090E short E (Kashmiri, Bihari, transcription)
    'ए', 'ऐ',
    // omit: U+0911 candra o (transcription)
    // omit: U+0912 short o (Kashmiri, Bihari)
    'ओ', 'औ',
    // 34 consonants (U+0915-U+0939)
    'क', 'ख', 'ग', 'घ', 'ङ', 'च', 'छ', 'ज', 'झ', 'ञ', 'ट', 'ठ', 'ड', 'ढ', 'ण', 'त',
    'थ', 'द', 'ध', 'न', 'प', 'फ', 'ब', 'भ', 'म', 'य', 'र', 'ल', 'ळ', 'व', 'श', 'ष',
    'स','ह',
    // omit: U+0934 letter LLLA (transcription)
    // 1 diacritic nukta
    '़',
    // 10 combining vowels (U+093E-U+094C)
    'ा', 'ि', 'ी', 'ु', 'ू', 'ृ',
    // omit: U+0944 vocalic rr (Sanskrit)
    // omit: U+0945 candra e (transcription)
    // omit: U+0946 short e (Kashmiri, Bihari, transcription)
    'े', 'ै',
    // omit: U+0949 candra o (transcription)
    // omit: U+094A short o (Kashmiri, Bihari, transcription)
    'ो', 'ौ',
    // omit: multiple diacritics for Kashmiri, Sanskrit, obsolete orthographies,
    // as well as old vocalics and Hindi nukta consonants (users should decompose them)
    // 1 diacritic virama
    '्',
    // 2 punctuators danda and double danda
    '।', '॥',
    // Space for 1 more, not sure which to include: expert help wanted!
    '\u{ffff}',
];

// Page 11 10: U+0000, then U+3131-U+3163 (initial part of KS X 1001 - 0x24 / 0xA4)
const HANGUL_COMPATIBILITY_JAMO : [char; 64] = [
    '\0', 'ㄱ', 'ㄲ', 'ㄳ', 'ㄴ', 'ㄵ', 'ㄶ', 'ㄷ', 'ㄸ', 'ㄹ', 'ㄺ', 'ㄻ', 'ㄼ', 'ㄽ', 'ㄾ', 'ㄿ',
    'ㅀ', 'ㅁ', 'ㅂ', 'ㅃ', 'ㅄ', 'ㅅ', 'ㅆ', 'ㅇ', 'ㅈ', 'ㅉ', 'ㅊ', 'ㅋ', 'ㅌ', 'ㅍ', 'ㅎ', 'ㅏ',
    'ㅐ', 'ㅑ', 'ㅒ', 'ㅓ', 'ㅔ', 'ㅕ', 'ㅖ', 'ㅗ', 'ㅘ', 'ㅙ', 'ㅚ', 'ㅛ', 'ㅜ', 'ㅝ', 'ㅞ', 'ㅟ',
    'ㅠ', 'ㅡ', 'ㅢ', 'ㅣ',
    // Space for 12 more, not sure which to include: expert help wanted!
    '\u{ffff}', '\u{ffff}', '\u{ffff}', '\u{ffff}',
    '\u{ffff}', '\u{ffff}', '\u{ffff}', '\u{ffff}',
    '\u{ffff}', '\u{ffff}', '\u{ffff}', '\u{ffff}',
];

// Page 11 11: U+0000, then U+FF61-U+FF9F (latter part of JIS-X-0201)
const HALFWIDTH_KANA : [char; 64] = [
    '\0', '｡', '｢', '｣', '､', '･', 'ｦ', 'ｧ', 'ｨ', 'ｩ', 'ｪ', 'ｫ', 'ｬ', 'ｭ', 'ｮ', 'ｯ',
    'ｰ', 'ｱ', 'ｲ', 'ｳ', 'ｴ', 'ｵ', 'ｶ', 'ｷ', 'ｸ', 'ｹ', 'ｺ', 'ｻ', 'ｼ', 'ｽ', 'ｾ', 'ｿ',
    'ﾀ', 'ﾁ', 'ﾂ', 'ﾃ', 'ﾄ', 'ﾅ', 'ﾆ', 'ﾇ', 'ﾈ', 'ﾉ', 'ﾊ', 'ﾋ', 'ﾌ', 'ﾍ', 'ﾎ', 'ﾏ',
    'ﾐ', 'ﾑ', 'ﾒ', 'ﾓ', 'ﾔ', 'ﾕ', 'ﾖ', 'ﾗ', 'ﾘ', 'ﾙ', 'ﾚ', 'ﾛ', 'ﾜ', 'ﾝ', 'ﾞ', 'ﾟ'
];

const RESERVED : [char; 64] = [
    '\u{ffff}', '\u{ffff}', '\u{ffff}', '\u{ffff}',
    '\u{ffff}', '\u{ffff}', '\u{ffff}', '\u{ffff}',
    '\u{ffff}', '\u{ffff}', '\u{ffff}', '\u{ffff}',
    '\u{ffff}', '\u{ffff}', '\u{ffff}', '\u{ffff}',
    '\u{ffff}', '\u{ffff}', '\u{ffff}', '\u{ffff}',
    '\u{ffff}', '\u{ffff}', '\u{ffff}', '\u{ffff}',
    '\u{ffff}', '\u{ffff}', '\u{ffff}', '\u{ffff}',
    '\u{ffff}', '\u{ffff}', '\u{ffff}', '\u{ffff}',
    '\u{ffff}', '\u{ffff}', '\u{ffff}', '\u{ffff}',
    '\u{ffff}', '\u{ffff}', '\u{ffff}', '\u{ffff}',
    '\u{ffff}', '\u{ffff}', '\u{ffff}', '\u{ffff}',
    '\u{ffff}', '\u{ffff}', '\u{ffff}', '\u{ffff}',
    '\u{ffff}', '\u{ffff}', '\u{ffff}', '\u{ffff}',
    '\u{ffff}', '\u{ffff}', '\u{ffff}', '\u{ffff}',
    '\u{ffff}', '\u{ffff}', '\u{ffff}', '\u{ffff}',
    '\u{ffff}', '\u{ffff}', '\u{ffff}', '\u{ffff}',
];

const CHINESE : [char; 64] = RESERVED;
const CHINESE_LO: char = '\u{4e00}';
const CHINESE_HI: char = '\u{9fff}';
const CHINESE_2BIT_TAG: usize = 0b11;
const CHINESE_4BIT_TAG: usize = 0b1100;

const PAGES : [[char; 64]; 16] = [
    LATIN,
    GREEK,
    CYRILLIC,
    HEBREW,

    ARABIC,
    RESERVED,
    RESERVED,
    RESERVED,

    DEVANAGARI,
    RESERVED,
    RESERVED,
    HANGUL_COMPATIBILITY_JAMO,

    CHINESE,
    RESERVED,
    RESERVED,
    HALFWIDTH_KANA
];

pub trait PackedValue
where
    Self: Copy,
    Self: ShlAssign<usize>,
    Self: BitOrAssign<Self>,
    Self: ::std::cmp::PartialOrd,
    Self: ::std::fmt::Debug,
    Self: ::std::fmt::LowerHex
{
    const NBITS: usize = size_of::<Self>() * 8;
    const NCHARS: usize = Self::NBITS / 6;
    const NTAGBITS: usize = Self::NBITS - (Self::NCHARS * 6);
    const NCHARBITS: usize = Self::NBITS - Self::NTAGBITS;
    // This is a bit ridiculous; I literally tried 4 different crates and every
    // trait I could find in the stdlib and it seems like there is some sort of
    // community-wide conspiracy to ensure the absence of generic truncating
    // casts.
    fn truncating_cast_from(i: usize) -> Self;

    // This also seems somewhat contorted to express via existing traits.
    fn most_significant_byte(self) -> u8;
}

impl PackedValue for u8 {
    fn truncating_cast_from(i: usize) -> u8 { i as u8 }
    fn most_significant_byte(self) -> u8 { self }
}

impl PackedValue for u16 {
    fn truncating_cast_from(i: usize) -> u16 { i as u16 }
    fn most_significant_byte(self) -> u8 { (self >> 8) as u8 }
}

impl PackedValue for u32 {
    fn truncating_cast_from(i: usize) -> u32 { i as u32 }
    fn most_significant_byte(self) -> u8 { (self >> 24) as u8 }
}

impl PackedValue for u64 {
    fn truncating_cast_from(i: usize) -> u64 { i as u64 }
    fn most_significant_byte(self) -> u8 { (self >> 56) as u8 }
}

impl PackedValue for u128 {
    fn truncating_cast_from(i: usize) -> u128 { i as u128 }
    fn most_significant_byte(self) -> u8 { (self >> 120) as u8 }
}

#[derive(PartialEq, Debug)]
pub enum EncodeError {
    TooLong,
    NoCodePageFor(char),
    PageUnavailable(usize),
    MissingFromPage(char)
}

fn chinese_15bit_delta(c: char) -> Option<usize> {
    if CHINESE_LO <= c && c <= CHINESE_HI {
        Some((c as usize) - (CHINESE_LO as usize))
    } else {
        None
    }
}

pub fn encode<N, IT>(i: IT) -> Result<N, EncodeError>
where
    N: PackedValue,
    IT: Iterator<Item = char>
{
    let mut pi = i.peekable();
    let mut out : N = N::truncating_cast_from(0);
    match pi.peek() {
        // Zero-length strings map to page 0, code 0.
        | None => Ok(out),
        | Some(&init) => {

            // First handle special case of Chinese characters, which are encoded as deltas.
            if N::NCHARBITS > 0 && chinese_15bit_delta(init) != None {
                let tag = if N::NTAGBITS == 2 { CHINESE_2BIT_TAG } else { CHINESE_4BIT_TAG };
                out |= N::truncating_cast_from(tag);
                let mut rembits : usize = N::NCHARBITS;
                for c in pi {
                    if rembits < 15 {
                        return Err(EncodeError::TooLong);
                    }
                    match chinese_15bit_delta(c) {
                        None => { return Err(EncodeError::MissingFromPage(c)); }
                        Some(delta) => {
                            out <<= 15;
                            // We encode delta+1 so that a delta of 0 is encoded as 1
                            // and we can still use 0-bytes to delimit the string.
                            out |= N::truncating_cast_from(delta + 1);
                            rembits -= 15;
                        }
                    }
                }
                // Pad remainder.
                out <<= rembits;
                return Ok(out)
            }

            // Pick page: just try each one, there are only 16.
            match PAGES.iter().position(|&p| p.binary_search(&init).is_ok()) {
                // No page means this string won't work.
                | None => Err(EncodeError::NoCodePageFor(init)),
                | Some(p) => {
                    let mut tag = p;
                    let mut rem : usize = N::NCHARS;
                    // Check and adjust tag by size.
                    if N::NTAGBITS == 2 {
                        // Tried a "secondary tag" when only
                        // using 2 tag bits, sorry!
                        if tag & 0b11 != 0 {
                            return Err(EncodeError::PageUnavailable(tag));
                        }
                        tag >>= 2;
                    }
                    // Set tag.
                    out |= N::truncating_cast_from(tag);
                    // Encode chars.
                    for c in pi {
                        if rem == 0 {
                            // String is too long.
                            return Err(EncodeError::TooLong);
                        }
                        match PAGES[p].binary_search(&c) {
                            // No code for c in page.
                            | Err(_) => return Err(EncodeError::MissingFromPage(c)),
                            // Got a code, use it!
                            | Ok(i) => {
                                out <<= 6;
                                out |= N::truncating_cast_from(i);
                                rem -= 1;
                            }
                        }
                    }
                    // Pad remainder.
                    out <<= 6 * rem;
                    Ok(out)
                }
            }
        }
    }
}

pub trait EncodeSixbit: Sized + Iterator<Item = char>
{
    fn encode_sixbit<N: PackedValue>(self) -> Result<N, EncodeError>;
}

impl<T> EncodeSixbit for T
where
    T: Sized,
    T: Iterator<Item = char>
{
    fn encode_sixbit<N: PackedValue>(self) -> Result<N, EncodeError> {
        encode::<N, Self>(self)
    }
}

pub struct DecodeSixbitIter<N: PackedValue> {
    tag: usize,
    tmp: N
}

impl<N> Iterator for DecodeSixbitIter<N>
where
    N: PackedValue
{
    type Item = char;
    fn next(self: &mut Self) -> Option<char> {
        if self.tag == CHINESE_4BIT_TAG {
            let ch0 = self.tmp.most_significant_byte();
            match ch0 {
                | 0 => None,
                | i => {
                    self.tmp <<= 8;
                    let ch1 = self.tmp.most_significant_byte();
                    self.tmp <<= 7;
                    let delta = ((i as u32) << 7) | ((ch1 as u32) >> 1);
                    char::from_u32((CHINESE_LO as u32) + delta - 1)
                }
            }
        } else {
            let mut ch = self.tmp.most_significant_byte();
            ch >>= 2;
            match ch {
                | 0 => None,
                | i => {
                    self.tmp <<= 6;
                    Some(PAGES[self.tag][i as usize])
                }
            }
        }
    }
}

pub trait DecodeSixbit
where Self: PackedValue
{
    fn decode_sixbit(self) -> DecodeSixbitIter<Self>;
}

impl<N> DecodeSixbit for N
where N: PackedValue
{
    fn decode_sixbit(self) -> DecodeSixbitIter<Self> {
        let mut tmp = self;
        let mut tag = self.most_significant_byte();
        tag >>= 8 - N::NTAGBITS;
        if N::NTAGBITS == 2 {
            tag <<= 2;
        }
        tmp <<= N::NTAGBITS;
        DecodeSixbitIter {
            tag: tag as usize,
            tmp,
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn misc_invariants() {
        // Check that pages are ordered by unicode ranges.
        for pair in PAGES.windows(2) {
            if pair[0][1] != '\u{ffff}' && pair[1][1] != '\u{ffff}' {
                if pair[0][1] >= pair[1][1] {
                    println!("mis-ordered page pair: {:?} >= {:?}", pair[0][1], pair[1][1]);
                }
                assert!(pair[0][1] < pair[1][1]);
            }
        }
        for p in PAGES.iter() {
            // Check that every page has a zero code, or is invalid.
            assert!(p[0] == '\0' || p[0] == '\u{ffff}');
            // Check that every page is sorted.
            for pair in p.windows(2) {
                if pair[0] != '\0' && pair[1] != '\0' &&
                    pair[0] != '\u{ffff}' && pair[1] != '\u{ffff}' {
                        if pair[0] >= pair[1] {
                            println!("mis-ordered char pair: {:?} >= {:?}", pair[0], pair[1]);
                        }
                        assert!(pair[0] < pair[1]);
                    }
            }
        }
    }

    fn round_trip<N:PackedValue>(s: &str) -> Result<N, EncodeError> {
        match s.chars().encode_sixbit::<N>() {
            Ok(enc) => {
                let dec:String = enc.decode_sixbit().collect();
                println!("roundtrip: {:?} => {:x} => {:?}", s, enc, dec);
                assert!(dec == s);
                Ok(enc)
            }
            err => err
        }
    }

    // For Latin we try a full-width, a not-full-width, and each of the
    // error conditions.
    #[test]
    fn test_latin() {
        // Full width.
        assert!(round_trip::<u128>("PRINTER_is_on_FIRE").is_ok());
        assert!(round_trip::<u64>("NO_CARRIER").is_ok());
        assert!(round_trip::<u32>("_CAT_").is_ok());
        assert!(round_trip::<u16>("OK").is_ok());
        assert!(round_trip::<u8>("1").is_ok());

        // Non-full-width.
        assert!(round_trip::<u128>("Printer_Working").is_ok());
        assert!(round_trip::<u64>("ATDT_123").is_ok());
        assert!(round_trip::<u32>("Uwu").is_ok());
        assert!(round_trip::<u16>("A").is_ok());
        assert!(round_trip::<u8>("").is_ok());

        // Error conditions: TooLong.
        assert!(round_trip::<u128>("PRINTER_FULLY_OPERATIONAL") == Err(EncodeError::TooLong));
        assert!(round_trip::<u64>("ATDT_123_4567") == Err(EncodeError::TooLong));
        assert!(round_trip::<u32>("aaaaaaa") == Err(EncodeError::TooLong));
        assert!(round_trip::<u16>("aba") == Err(EncodeError::TooLong));
        assert!(round_trip::<u8>("OOH") == Err(EncodeError::TooLong));

        // Error conditions: NoCodePageFor.
        assert!(round_trip::<u128>("©2018") == Err(EncodeError::NoCodePageFor('©')));

        // Error conditions: PageUnavailable.
        assert!(round_trip::<u128>("ΨΩ") == Err(EncodeError::PageUnavailable(1)));

        // Error conditions: MissingFromPage.
        assert!(round_trip::<u64>("sh@rk") == Err(EncodeError::MissingFromPage('@')));
    }

    fn check_order<N:PackedValue>(a: &str, b: &str) {
        assert!(a < b);
        assert!(a.chars().encode_sixbit::<N>().unwrap() < b.chars().encode_sixbit::<N>().unwrap());
    }

    #[test]
    fn sorting() {
        // Check encoding order preservation within pages.
        check_order::<u32>("", "AB");
        check_order::<u64>("abcd", "abcde");
        check_order::<u64>("abcde", "abcdf");
        check_order::<u64>("α", "αβγ");
        check_order::<u64>("αβ", "αβγ");
        check_order::<u64>("αβγ", "αβδ");
        check_order::<u64>("怎么", "怎么样");
        check_order::<u64>("以前", "以后");
        // Check encoding order preservation across pages.
        check_order::<u64>("abc", "αβγ");
        check_order::<u64>("αβγ", "абв");
        check_order::<u64>("абв", "אבג");
        check_order::<u64>("אבג", "ابة");
        check_order::<u64>("ابة", "कखग");
        check_order::<u64>("कखग", "ㄱㄲㄳ");
        check_order::<u64>("ㄱㄲㄳ", "合伙人");
        check_order::<u64>("合伙人", "ｦｧｨ");
    }

    // For non-Latin scripts we just check a word at each width
    // to make sure they work.

    #[test]
    fn test_greek() {
        // Non-primary tag: only available in u64 and u16 forms.
        assert!(round_trip::<u64>("αλήθεια").is_ok());
        assert!(round_trip::<u16>("γη").is_ok());
    }

    #[test]
    fn test_cyrillic() {
        //Non-primary tag: available only in u64 and u16 forms.
        assert!(round_trip::<u64>("содержать").is_ok());
        assert!(round_trip::<u16>("же").is_ok());
    }

    #[test]
    fn test_hebrew() {
        // Non-primary tag: only available in u64 and u16 forms.
        assert!(round_trip::<u64>("לעשות").is_ok());
        assert!(round_trip::<u16>("כל").is_ok());
    }

    #[test]
    fn test_arabic() {
        // Primary tag: available in all forms.
        assert!(round_trip::<u128>("محافظت").is_ok());
        assert!(round_trip::<u64>("العاصمة").is_ok());
        assert!(round_trip::<u32>("البعض").is_ok());
        assert!(round_trip::<u16>("از").is_ok());
        assert!(round_trip::<u8>("و").is_ok());
    }

    #[test]
    fn test_devanagari() {
        // Primary tag: available in all forms.
        assert!(round_trip::<u128>("किंकर्तव्यविमूढ़").is_ok());
        assert!(round_trip::<u64>("आवश्यकता").is_ok());
        assert!(round_trip::<u32>("सपना").is_ok());
        assert!(round_trip::<u16>("पल").is_ok());
        assert!(round_trip::<u8>("आ").is_ok());
    }

    #[test]
    fn test_chinese() {
        // Special-case 15-bit primary tag: only forms >=32 bits.
        assert!(round_trip::<u128>("高速火车站").is_ok());
        assert!(round_trip::<u64>("合伙人").is_ok());
        assert!(round_trip::<u32>("同事").is_ok());
    }

    #[test]
    fn test_compatibility_hangul_jamo() {
        // Non-primary tag: only available in u64 and u16 forms.
        assert!(round_trip::<u64>("ㅇㅜㅁㅈㅣㄱㅇㅣㅁ").is_ok());
        assert!(round_trip::<u16>("ㅅㅜ").is_ok());
    }

    #[test]
    fn test_halfwidth_kana() {
        // Non-primary tag: only available in u64 and u16 forms.
        assert!(round_trip::<u64>("ｲｸﾂｶﾉ").is_ok());
        assert!(round_trip::<u16>("ﾔﾙ").is_ok());
    }
}
