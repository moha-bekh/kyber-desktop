use log::error;

pub(crate) fn parse_bitrate(mut bitrate: String) -> Option<u32> {
    let Some(last_char) = bitrate.chars().last() else {
        error!("Empty bitrate");
        return None;
    };

    let scale = match last_char {
        'M' => Some(1_000_000),
        'K' => Some(1_000),
        _ => None,
    };

    if scale.is_some() {
        bitrate = bitrate[0..bitrate.len() - 1].into();
    }

    let bitrate: u32 = match bitrate.parse() {
        Ok(bitrate) => bitrate,
        Err(err) => {
            error!("Invalid bitrate {bitrate}: {err:?}");
            return None;
        }
    };

    match scale {
        Some(scale) => Some(bitrate * scale),
        None => Some(bitrate),
    }
}

#[cfg(test)]
mod tests {
    use crate::parse_bitrate;

    #[test]
    fn bitrate_parser() {
        assert_eq!(parse_bitrate("".into()), None);
        assert_eq!(parse_bitrate("100aM".into()), None);
        assert_eq!(parse_bitrate("100zdee".into()), None);
        assert_eq!(parse_bitrate("100".into()), Some(100));
        assert_eq!(parse_bitrate("1K".into()), Some(1_000));
        assert_eq!(parse_bitrate("230K".into()), Some(230_000));
        assert_eq!(parse_bitrate("1M".into()), Some(1_000_000));
        assert_eq!(parse_bitrate("230M".into()), Some(230_000_000));
    }
}
