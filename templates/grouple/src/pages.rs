use aidoku::alloc::String;
use aidoku::alloc::vec::Vec;
use alloc::string::ToString;

/// Extract image URLs from a Grouple reader page.
///
/// The reader inlines a JS call like
///   `rm_h.readerInit(0, 0, [['','https://cdn/','/img/path/',"01.jpg",800,1200], ...], …);`
/// or the older `rm_h.readerDoInit(...)`. Each inner array starts with three
/// quoted strings (single + single + double) which combine into the page URL.
pub fn extract_pages(html: &str, base_url: &str) -> Vec<String> {
	let segment = locate_reader_segment(html);
	let Some(segment) = segment else {
		return Vec::new();
	};

	let triples = scan_triples(segment);
	let mut out = Vec::with_capacity(triples.len());
	for (cdn, dir, file) in triples {
		let url = compose_url(&cdn, &dir, &file, base_url);
		if !url.is_empty() {
			out.push(url);
		}
	}
	out
}

fn locate_reader_segment(html: &str) -> Option<&str> {
	let markers = ["rm_h.readerInit(", "rm_h.readerDoInit("];
	let start = markers
		.iter()
		.filter_map(|m| html.find(m).map(|idx| idx + m.len()))
		.min()?;
	let rest = &html[start..];
	let end = rest.find(");")?;
	Some(&rest[..end])
}

fn scan_triples(segment: &str) -> Vec<(String, String, String)> {
	let bytes = segment.as_bytes();
	let mut out = Vec::new();
	let mut i = 0usize;
	while i < bytes.len() {
		// Look for an inner array opener: `'` directly after `[` (with optional whitespace).
		// Then read string + ',' + string + ',' + string.
		if bytes[i] == b'\'' {
			let res = read_triple(bytes, i);
			if let Some((s1, s2, s3, after)) = res {
				out.push((s1, s2, s3));
				i = after;
				continue;
			}
		}
		i += 1;
	}
	out
}

fn read_triple(bytes: &[u8], start: usize) -> Option<(String, String, String, usize)> {
	let (s1, after1) = read_quoted(bytes, start, b'\'')?;
	let after1 = skip_separators(bytes, after1);
	let (s2, after2) = read_quoted(bytes, after1, b'\'')?;
	let after2 = skip_separators(bytes, after2);
	// Third token is allowed to be either single- or double-quoted in practice.
	let opener = bytes.get(after2).copied()?;
	if opener != b'"' && opener != b'\'' {
		return None;
	}
	let (s3, after3) = read_quoted(bytes, after2, opener)?;
	Some((s1, s2, s3, after3))
}

fn read_quoted(bytes: &[u8], at: usize, quote: u8) -> Option<(String, usize)> {
	if bytes.get(at).copied()? != quote {
		return None;
	}
	let start = at + 1;
	let end = bytes[start..].iter().position(|&b| b == quote)?;
	let s = core::str::from_utf8(&bytes[start..start + end]).ok()?;
	Some((s.to_string(), start + end + 1))
}

fn skip_separators(bytes: &[u8], mut i: usize) -> usize {
	while i < bytes.len() {
		match bytes[i] {
			b' ' | b'\t' | b'\n' | b'\r' | b',' => i += 1,
			_ => break,
		}
	}
	i
}

fn compose_url(cdn: &str, dir: &str, file: &str, base_url: &str) -> String {
	// Mirror keiyoushi's logic.
	let mut url: String = if cdn.is_empty() && dir.is_empty() && file.starts_with("/static/") {
		alloc::format!("{}{}", base_url, file)
	} else if cdn.is_empty() && file.starts_with("/static/") {
		alloc::format!("{}{}", base_url, file)
	} else if dir.ends_with("/manga/") {
		alloc::format!("{}{}", cdn, file)
	} else if dir.is_empty() {
		alloc::format!("{}{}", cdn, file)
	} else {
		alloc::format!("{}{}{}", dir, cdn, file)
	};
	if !url.contains("://") {
		url = if let Some(stripped) = url.strip_prefix("//") {
			alloc::format!("https://{}", stripped)
		} else {
			alloc::format!("https:{}", url)
		};
	}
	if url.contains("one-way.work") {
		if let Some(idx) = url.find('?') {
			url.truncate(idx);
		}
	}
	url.replace("//resh", "//h")
}
