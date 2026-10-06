use super::*;

fn accepts(input: &str) {
    let parsed = RenderZpl::parse(input.into()).unwrap_or_else(|e| panic!("{input:?}: {e}"));
    assert_eq!(parsed.as_str(), input);
}

fn rejects(input: &str) {
    assert!(
        RenderZpl::parse(input.into()).is_err(),
        "accepted {input:?}"
    );
}

#[test]
fn accepts_self_contained_rendering_and_preserves_bytes() {
    for input in [
        "^XA^XZ",
        " \n^XA\n^PW812^LL1218^LH0,0^FO20,30^A0N,32,32^FDHello World!^FS\n^XZ\n",
        "^XA^CI28^CF0,24^FO0,0^FDcafé^FS^XZ",
        "^XA^FO20,20^BY2,2.5,100^BCN,100,Y,N,N,A^FD123456^FS^XZ",
        "^XA^BQN,2,4^FDLA,https://example.org^FS^XZ",
        "^XA^FO10,10^GB100,50,2,B,4^FR^FS^FO20,80^GC30,2^FS^XZ",
        "^XA^FXAn ordinary comment\n^FB300,3,0,L,0^FDTwo\\&lines^FS^XZ",
        "^XA^FH^FDliteral _5EXA_7EHS_0A_21 U1 getvar _22allcv_22^FS^XZ",
        "^XA^FH;^FD;5EWD;7EHS^FS^XZ",
        "^XA^GFA,2,2,1,FF00^FS^XZ",
        "^XA^GFA,4,4,2,J0:^FS^XZ",
        "^XA^GFA,2,2,1,:B64:/wA=:673B^FS^XZ",
    ] {
        // ^FH output is inert field text, including encoded command prefixes
        // (Zebra guide ^FD/^FH pp.190/193), not a new command stream.
        accepts(input);
    }
}

#[test]
fn rejects_unparseable_unknown_and_unterminated_streams() {
    for input in [
        "",
        "hello",
        "^",
        "^X",
        "^XA^A",
        "^XA",
        "^XZ",
        "^XA^FDhello^XZ",
        "^XA^XZ^XA^XZ",
        "^XA^XA^XZ",
        "garbage^XA^XZ",
        "^XA^XZgarbage",
        "^XA^ZZ^XZ",
        "^xa^xz",
        "^XA^FSgarbage^XZ",
        "^XA^FH^FD_0^FS^XZ",
        "^XA^FH^FD_GG^FS^XZ",
        "^XA^FHabc^FDx^FS^XZ",
        "^XA^GFA,2,2,1,FF^FS^XZ",
        "^XA^GFA,1,2,1,FF00^FS^XZ",
        "^XA^GFA,2,2,1,:B64:/wA=:0000^FS^XZ",
        "^XA^GFA,2,2,1,:Z64:bad^XZ",
        "^XA^GFA,2,2,1,FF00garbage^FS^XZ",
        "^XA^GFA,1,1,1,FF^XZ",
    ] {
        rejects(input);
    }
}

#[test]
fn rejects_device_operations_queries_and_executable_stored_formats() {
    // Guide: object/directory inspection ^WD/^XF/^XG pp.361/372/373,
    // named/persistent fonts ^A@/^CW pp.62/168, retained bitmap ^MC p.300.
    for command in [
        "^WD*:*.*",
        "^XFR:PRIVATE.ZPL",
        "^ISR:OLD.GRF",
        "^IDR:*.*",
        "^DFFILE.ZPL",
        "^CW0,R:PRIVATE.TTF",
        "^FL0,1,2",
        "^LF",
        "^HFR:PRIVATE.ZPL",
        "^HGR:PRIVATE.GRF",
        "^HH",
        "^HV1",
        "^HW",
        "^HY",
        "^HZ",
        "~HS",
        "~HQES",
        "~WC",
        "~JA",
        "~JR",
        "~JC",
        "^JUS",
        "^JUF",
        "^JUN",
        "^KP1234",
        "^ND1,192.0.2.1",
        "^RFR,H,0,8",
        "^RFW,H",
        "^PR14",
        "^MD30",
        "^PQ99",
        "^PN10",
        "^PH",
        "^MCN",
        "~DGSECRET,1,1,FF",
        "~DYSECRET,A,G,1,1,FF",
        "~DB",
        "~DU",
        "~JI",
        "^JI",
    ] {
        rejects(&format!("^XA{command}^FS^XZ"));
    }
}

#[test]
fn rejects_framing_and_other_language_bypasses() {
    for input in [
        "^XA^CC!/FO1,2!HWE:*!XZ",
        "^XA^CT!^FXcomment!HS^FS^XZ",
        "^XA^CD;^FO1;2^XZ",
        "~CC! !XA!WD!XZ",
        "\x02^FDhello\x0f\x03",
        "^XA^FDhello\x00~HS^FS^XZ",
        "^XA^FXcomment\n! U1 getvar \"allcv\"\n^FS^XZ",
        "^XA^FDhello\n! U1 getvar \"allcv\"\n^FS^XZ",
        "! U1 getvar \"allcv\"\n^XA^XZ",
        "^XA^FS\n! U1 getvar \"allcv\"\n^XZ",
        "^XA^XZ\n! U1 getvar \"allcv\"",
        "^XA^GFB,3,3,1,^WD^FS^XZ",
        "^XA^GFC,3,3,1,^WD^FS^XZ",
        "^XA^GFB,999999999,999999999,1,^WD^FS^XZ",
        "^XA^FXcomment^WD^FS^XZ",
        "^XA^FDsafe^FS~HS^XZ",
    ] {
        rejects(input);
    }
}

#[test]
fn rejects_bad_suffix_before_authorizing_any_prefix() {
    for suffix in ["^", "^X", "~HS", "^WD", "garbage", "\x1b", "^XA^XZ"] {
        rejects(&format!("^XA^FO20,20^FDvalid field^FS^XZ{suffix}"));
    }
}

#[test]
fn rendering_operands_are_left_to_firmware() {
    // Admission checks command effects and framing, not firmware's operand grammar.
    for input in [
        "^XA^FOnope,20^XZ",
        "^XA^FO1,2,0,4^XZ",
        "^XA^PW999999999999999999^XZ",
        "^XA^FO1e2,0^XZ",
        "^XA^POZ^XZ",
        "^XA^CI14^XZ",
        "^XA^CI28,65,66^XZ",
        "^XA^BXN,4,200,0,0,6,!^FD!d065^FS^XZ",
        "^XA^PA1,1,1,1^FM10,20,30,40^TB N,100,100^FDtext^FS^XZ",
        "^XA^AZN,32,32^FDtext^FS^XZ",
    ] {
        accepts(input);
    }
    // Guide barcodes pp.64-150: explicit known list, not a B* wildcard.
    for code in [
        "B0", "B4", "B5", "BB", "BD", "BF", "BI", "BJ", "BK", "BL", "BM", "BO", "BP", "BR", "BS",
        "BT", "BZ",
    ] {
        accepts(&format!("^XA^{code}N^FD123456^FS^XZ"));
    }
    rejects("^XA^B6^XZ");
    rejects("^XA^BXN,4,200,0,0,6,~^FDx^FS^XZ");
    rejects("^XA^FO1,2\n! U1 getvar \"allcv\"\n^XZ");
}

#[test]
fn external_content_and_variable_fields_are_accepted() {
    // Guide ^A@ p.62, ^IL/^IM pp.247-248, ^XG p.373; clock/serial fields
    // are rendering commands regardless of the shared cache/recovery policy.
    for command in [
        "^A@N,30,30,R:FONT.TTF",
        "^XGR:IMAGE.GRF,1,1",
        "^ILR:IMAGE.GRF",
        "^IMR:IMAGE.GRF",
        "^FC",
        "^FE",
        "^FN1",
        "^SN1",
        "^SF",
    ] {
        let input = format!("^XA{command}^FS^XZ");
        accepts(&input);
    }
}

#[test]
fn rendering_resource_limits_remain_bounded() {
    rejects(&format!("^XA^FD{}^FS^XZ", "x".repeat(1_048_576)));
    rejects("^XA^GFA,25001,25001,1,:^FS^XZ");
    let graphic = format!("^GFA,25000,25000,100,{}^FS", "00".repeat(25_000));
    accepts(&format!("^XA{}^XZ", graphic.repeat(10)));
    rejects(&format!("^XA{}^XZ", graphic.repeat(11)));
}
