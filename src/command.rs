use core::ops::{Deref, DerefMut};

/// Parsed representation of ZPL commands (operations).
///
/// Given a machine definition/configuration and a list of zpl commands, one can generate (format)
/// a label (document).
pub enum Command {
    /// `^Afo,h,w`, specify font for use in text field
    A {
        font_name: Option<u8>,
        orientation: Option<u8>,
        height: Option<u8>,
        width: Option<u8>,
    },
    /// `^A@o,h,w,d:f.x`, specify font with full name for use in text field
    Aat {
        orientation: Option<u8>,
        height: Option<u8>,
        width: Option<u8>,
        // `d:f.x`
        // fixme: theoretically parts of this are optional
        font_name: Option<String>,
    },
    /// `^B0a,b,c,d,e,f,g`
    B0 {
        a: u8,
        b: u8,
        c: u8,
        d: u8,
        e: u8,
        f: u8,
        g: u8,
    },
    /// `^B1o,e,h,f,g`
    B1 {
        orientation: u8,
        check_digit: u8,
        height: u8,
        print_interpretation_line: bool,
        print_interpretation_line_above_code: bool,
    },
    /// `^B2o,h,f,g,e,j`
    ///
    /// XXX: format shows a `j`, but parameter description omits it.
    B2 {
        orientation: u8,
        height: u8,
        print_interpretation_line: bool,
        print_interpretation_line_above_code: bool,
        calculate_and_print_check_digit: bool,
    },

    /// `^B3o,e,h,f,g`
    B3 {
        orientation: u8,
        check_digit: u8,
        height: u8,
        print_interpretation_line: bool,
        print_interpretation_line_above_code: bool,
    },

    /// `^B4o,h,f,m`
    B4 {
        orientation: u8,
        height: u8,
        print_interpretation_line: bool,
        starting_mode: u8,
    },

    /// `^B5o,h,f,g`
    B5 {
        orientation: u8,
        height: u8,
        print_interpretation_line: bool,
        print_interpretation_line_above_code: bool,
    },

    /// `^B7o,h,s,c,r,t`
    B7 {
        orientation: u8,
        height: u8,
        security_level: u8,
        columns: u8,
        rows: u8,
        truncation: u8,
    },

    /// `^B8o,h,f,g`
    B8 {
        orientation: u8,
        height: u8,
        print_interpretation_line: bool,
        print_interpretation_line_above_code: bool,
    },

    /// `^B9o,h,f,g,e`
    B9 {
        orientation: u8,
        height: u8,
        print_interpretation_line: bool,
        print_interpretation_line_above_code: bool,
        print_check_digit: bool,
    },

    /// `^BAo,h,f,g,e`
    BA {
        orientation: u8,
        height: u8,
        print_interpretation_line: bool,
        print_interpretation_line_above_code: bool,
        print_check_digit: bool,
    },

    /// `^BBo,h,s,c,r,m`
    BB {
        orientation: u8,
        height: u8,
        security_level: u8,
        columns: u8,
        rows: u8,
        mode: u8,
    },

    /// `^BCo,h,f,g,e,m`
    BC {
        orientation: u8,
        height: u8,
        print_interpretation_line: bool,
        print_interpretation_line_above_code: bool,
        check_digit: bool,
        mode: u8,
    },

    /// `^BDm,n,t`, UPS MaxiCode Barcode
    BD {
        mode: u8,
        symbol_number: u8,
        total_symbols: u8,
    },

    /// `^BEo,h,f,g`, EAN-13 Barcode
    BE {
        orientation: u8,
        height: u8,
        print_interpretation_line: bool,
        print_interpretation_line_above_code: bool,
    },

    /// `^BFo,h,m`, MicroPDF417 Barcode
    BF {
        orientation: u8,
        height: u8,
        mode: u8,
    },

    /// `^BIo,h,f,g`, Industrial 2 of 5 Barcode
    BI {
        orientation: u8,
        height: u8,
        print_interpretation_line: bool,
        print_interpretation_line_above_code: bool,
    },

    /// `^BJo,h,f,g`, Standard 2 of 5 Barcode
    BJ {
        orientation: u8,
        height: u8,
        print_interpretation_line: bool,
        print_interpretation_line_above_code: bool,
    },

    /// `^BKo,e,h,f,g,k,l`, ANSI Codabar Barcode
    BK {
        orientation: u8,
        check_digit: u8,
        height: u8,
        print_interpretation_line: bool,
        print_interpretation_line_above_code: bool,
        start_character: u8,
        stop_character: u8,
    },

    /// `^BLo,h,g`, LOGMARS Barcode
    BL {
        orientation: u8,
        height: u8,
        print_interpretation_line_above_code: bool,
    },

    /// `^BMo,e,h,f,g,e2`, MSI Barcode
    BM {
        orientation: u8,
        check_digit_mode: u8,
        height: u8,
        print_interpretation_line: bool,
        print_interpretation_line_above_code: bool,
        show_check_digit: bool,
    },

    /// `^BOa,b,c,d,e,f,g`, Aztec Barcode
    BO {
        orientation: u8,
        magnification_factor: u8,
        extended_channel_interpretation_code: u8,
        error_correction: u16,
        menu_symbol: u8,
        number_of_symbols: u8,
        id_field: u8,
    },

    /// `^BPo,e,h,f,g`, Plessey Barcode
    BP {
        orientation: u8,
        print_check_digit: u8,
        height: u8,
        print_interpretation_line: bool,
        print_interpretation_line_above_code: bool,
    },

    /// `^BQa,b,c,d,e`, QR Code Barcode
    BQ {
        orientation: u8,
        model: u8,
        magnification_factor: u8,
        error_correction: u8,
        mask_value: u8,
    },

    /// `^BRa,b,c,d,e,f`, GS1 Databar (formerly Reduced Space Symbology) Barcode
    BR {
        orientation: u8,
        symbology: u8,
        magnification_factor: u8,
        separator_height: u8,
        height: u8,
        segment_width: u8,
    },

    /// `^BSo,h,f,g`, UPC/EAN Extensions.
    ///
    /// Used with `^BU` and `^B9`.
    BS {
        orientation: u8,
        height: u8,
        print_interpretation_line: bool,
        print_interpretation_line_above_code: bool,
    },

    /// `^BTo,w1,r1,h1,w2,h2`, TLC39 Barcode
    BT {
        orientation: u8,
        width1: u8,
        bar_ratio1: u8,
        height1: u8,
        width2: u8,
        height2: u8,
    },

    /// `^BUo,h,f,g,e`, UPC-A Barcode
    BU {
        orientation: u8,
        height: u8,
        print_interpretation_line: bool,
        print_interpretation_line_above_code: bool,
        print_check_digit: bool,
    },

    /// `^BXo,h,s,c,r,f,g,a`, Data Matrix Barcode
    BX {
        orientation: u8,
        height: u8,
        quality: u8,
        columns: u8,
        rows: u8,
        format: u8,
        escape_sequence_control_character: u8,
        aspect_ratio: u8,
    },

    /// `^BYw,r,h`, Barcode field Default
    BY { width: u8, ratio: u8, height: u8 },

    /// `^BZo,h,f,g,t`, POSTAL Barcode
    BZ {
        orientation: u8,
        height: u8,
        print_interpretation_line: bool,
        print_interpretation_line_above_code: bool,
        r#type: u8,
    },

    /// `^CCx`, `~CCx`, Change Caret
    CC { x: u8 },

    /// `^CDa`, `~CDa`, Change Delimiter
    CD { a: u8 },

    /// `^CFf,h,w`, Change the alphanumeric default font
    CF {
        /// A-Z, 0-9. Default is 'A'
        font: u8,
        /// 0 to 32000, default 9
        height: u16,
        /// 0 to 32000, default 5  or last permanent setting
        width: u16,
    },

    /// `^CIa,s1,d1,s2,d2,...`
    CI {
        /// 0-36 (various exclusions), default 0.
        a: u8,

        /// up to 256 s,d pairs, each item is 0 to 255
        source_dest: Vec<(u8, u8)>,
    },

    /// `^CMa,b,c,d`
    CM {
        a: String,
        b: String,
        c: String,
        d: String,
        e: String,
    },

    /// `^CNa` cycle the media cutter
    CN { cut_mode_override: u8 },

    /// `^COa,b,c`, cache on
    CO {
        cache_on: bool,
        memory_to_add_kilobytes: u16,
        cache_type: u8,
    },

    /// `^CPa`, Remove label
    CP { kiosk_present_mode: u8 },

    /// `^CTa`, `~CTa`, Change Tilde
    CT { a: u8 },

    /// `^CVa`, code validation
    CV { a: bool },

    /// `^CWa,d:o.x`, Font identifier
    CW { font_letter: u8, font_path: String },

    /// `~DBd:o.x,a,h,w,base,space,#char,(c),data`, download bitmap font
    DB {
        font_path: String,
        orientation: u8,
        /// 1 to 32000
        height_max: u16,
        /// 1 to 32000
        width_max: u16,
        /// 1 to 32000
        base: u16,
        /// 1 to 32000
        space: u16,
        char_count: u8,
        copyright_holder: String,
    },

    /// `~DEd:o.x,s,data`, download encoding
    DE {
        table_path: String,
        table_bytes: u32,
        data: Vec<u8>,
    },

    /// `^DFd:o.x`, download format
    DF { format_path: String },

    /// `~DGd:o.x,t,w,data`, download graphics
    DG {
        path: String,
        bytes: u32,
        bytes_per_row: u32,
        data: Vec<u8>,
    },

    /// `~DN`, exit graphics mode, return to print mode
    DN,

    /// `~DSd:o.x,s,data` download intellifont (download scalable font)
    DS {
        font_path: String,
        bytes: u32,
        data: Vec<u8>,
    },

    /// `~DTd:o.x,s,data`, download bounded truetype font
    DT {
        font_path: String,
        bytes: u32,
        data: Vec<u8>,
    },

    /// `~DUd:o.x,s,data`, download unbounded truetype font
    DU {
        font_path: String,
        bytes: u32,
        data: Vec<u8>,
    },

    /// `~DYd:f,b,x,t,w,data`, download objects
    // XXX: `x` is the file extention and it's in a wierd possition here, watch out!
    DY {
        path: String,
        format: u8,
        bytes: u32,
        bytes_per_row: u32,
        data: Vec<u8>,
    },

    /// `^FBa,b,c,d,e`, Field Block
    FB {
        width: u32,
        /// 1 to 9999, default 0
        max_lines: u16,
        /// -9999 to 9999, default 0
        change_space_between_lines: i16,
        text_justification: u8,
        /// 0 to 9999, default 0
        hanging_indent: u16,
    },

    /// `^FCa,b,c`, Field Clock
    FC {
        primary_clock_indicator_character: u8,
        secondary_clock_indicator_character: u8,
        tertiary_clock_indicator_character: u8,
    },

    /// `^FDa`
    FD { data: String },

    /// `FEa`
    FE { a: u8 },

    /// `^FHa`, Field Hexadecimal Indicator
    FH { a: u8 },

    /// `^FL<ext>,<base>,<link>`
    FL {
        ext: String,
        base: String,
        link: String,
    },

    /// `^FMx1,y1,x2,y2,...`
    FM { x_y_pairs: Vec<(u16, u16)> },

    /// `^FN#"a"
    FN { num: u16, a: String },

    /// `^FOx,y,z`
    FO { x: u16, y: u16, z: u8 },

    /// `^FPd,g`
    FP { direction: u8, gap: u16 },

    /// `^FR`, field reverse print
    FR,

    /// `^FS`, field separator
    FS,

    /// `^FTx,y,z`
    FT { x: u16, y: u16, z: u8 },

    /// `^FVa`, field variable
    FV { a: u8 },

    /// `^FWr,z`
    FW { rotate: u8, z: u8 },

    /// `^FXc`, comment
    FX { comment: String },

    /// `^GBw,h,t,c,r`, graphic box
    GB {
        width: u16,
        height: u16,
        thickness: u8,
        corner_rounding: u8,
        border: u8,
    },

    /// `^GCd,t,c`, graphic circle
    GC {
        diameter: u16,
        thickness: u8,
        line_color: BlackOrWhite,
    },

    /// `^GDw,h,t,c,o`, graphic diagonal line
    GD {
        /// 3 to 32000, default to value of `thickness` or 3
        width: u16,
        /// 3 to 32000, default to value of `thickness` or 3
        height: u16,
        /// 1 to 32000, default 1
        thickness: u16,
        line_color: BlackOrWhite,
        orientation: u8,
    },

    /// `^GEw,h,t,c`, graphic ellipse
    GE {
        /// 3 to 4095, default `thickness` or 1
        // FIXME: default 1 is out of range in docs. Examine!
        width: u16,
        /// 3 to 4095, default `thickness` or 1
        // FIXME: default 1 is out of range in docs. Examine!
        height: u16,
        /// 2 to 4095, default 1
        thickness: u16,
        line_color: BlackOrWhite,
    },

    /// `^GFa,b,c,d,data`, Grapic Field
    GF {
        compression_type: u8,
        /// 1 to 99999
        binary_byte_count: u32,
        graphic_field_count: u32,
        bytes_per_row: u32,
        data: Vec<u8>,
    },

    /// `^GSo,h,w`, graphic symbol
    GS {
        orientation: u8,
        height: u16,
        width: u16,
    },

    /// `~HB`, battery status
    HB,

    /// `~HD`, head diagnostic
    HD,

    /// `^HFd,o,x`, Host Format
    HF {
        device: String,
        name: String,
        extension: String,
    },

    /// `^HGd:o.x`, Host Graphic
    HG {
        device: String,
        orientation: u8,
        extension: String,
    },

    /// `^HH`, Configuration Label Return
    HH,

    /// `~HI`, Host Identification
    HI,

    /// `~HM`, Host RAM Status
    HM,

    /// `~HQ<query-type>`, Host Query
    HQ { query_type: String },

    /// `~HS`, Host Status Return
    HS,

    /// `^HT`, Host Linked Fonts List
    HT,

    /// `~HU`, Return ZebraNet Alert Configuration
    HU,

    /// `^HV#,n,h,t,a`, Host Verification
    HV {
        number: u16,
        n: u8,
        h: u16,
        t: u16,
        a: u8,
    },

    /// `^HWd:o.x`, Host Directory List
    HW {
        device: String,
        name: String,
        extension: String,
    },

    /// `^HYd:o.x`, Upload Graphics
    HY {
        device: String,
        object_name: String,
        extension: String,
    },

    /// `^HZb`, Display Description Information
    HZ { b: u8 },

    /// `^IDd:o.x`, object delete
    ID {
        device: String,
        object_name: String,
        extension: String,
    },

    /// `^ILd:o.x`, Image load
    IL {
        device: String,
        object_name: String,
        extension: String,
    },

    /// `^IMd:o.x`, Image move
    IM {
        device: String,
        object_name: String,
        extension: String,
    },

    /// `^ISd:o.x,p`, Image save
    IS {
        device: String,
        object_name: String,
        extension: String,
        print_image_after_storing: bool,
    },

    /// `~JA`, Cancel All
    JA,

    /// `^JBa`, initialize flash memory
    JB { device: u8 },

    /// `~JB`, reset optional memory
    JB,

    /// `~JC`, set media sensor calibration
    JC,

    /// `~JD`, enable communications diagnostics
    JD,

    /// `~JE`, disable communications diagnostics
    JE,

    /// `~JF`, set battery condition
    JF { pause_on_low_voltage: bool },

    /// `~JG`, Graphing Sensor Calibration
    JG,

    /// `^JHa,b,c,d,e,f,g,h,i,j`
    JH {
        early_warning_media: u8,
        /// 100 to 9999, default 900
        labels_per_roll: u16,
        media_replaced: bool,
    },

    /// `~JId:o.x,b,c,d`, Start ZBI (Zebra BASIC Interpreter)
    JI {
        device: String,
        name: String,
        extension: String,

        console_enable: bool,
        echo: bool,
        memory_allocation: u32,
    },

    /// `~JI` ????

    /// `^JJa,b,c,d,e,f`, set auxiliary port
    JJ {
        operational_mode: u8,
        application_mode: u8,
        application_mode_start_signal_print: u8,
        application_label_error_mode: u8,
        reprint_mode: u8,
        ribbon_low_mode: bool,
    },

    /// `~JL`, set label length
    JL,

    /// `^JMn`, set dots per millimeter
    JM { dots_per_millimeter: u8 },

    /// `~JN`, head test fatal
    JN,

    /// `~JO`, head test non-fatal
    JO,

    /// `~JP`, pause and cancel format
    JP,

    /// `~JQ`, terminate Zebra BASIC Interpreter
    JQ,

    /// `~JR`, power on reset
    JR,

    /// `~JSa`, sensor select
    JS { sensor_selection: u8 },

    /// `~JSb`, change backfeed sequence
    JSx {
        backfeed_order_in_relation_to_printing: String,
    },

    /// `^JT####,a,b,c`, Head test interval
    JT {
        interval: u16,
        manually_select_elements: bool,
        first_element_to_check: u16,
        last_element_to_check: u16,
    },

    /// `^JUa`, Configuration Update
    JU { active_configuration: u8 },

    /// `^JWt`, Set ribbon tension
    JW { tension: u8 },

    /// `~JX`, cancel current partially input format
    JX,

    /// `^JZ`, reprint after error
    JZ { reprint_after_error: bool },

    /// `~KB`, kill battery (battery discharge mode)
    KB,

    /// `^KDa`, select date and time format (for real time clock)
    KD { date_format: u8 },

    /// `^KLa`, Define language
    KL { language: u8 },

    /// `^KNa,b`, Define printer name
    KN { name: String },

    /// `^KPa,b`, Define password
    KP { password: String, level: u8 },

    /// `^KVa,b,c,d,e`, Kiosk values
    KV {
        kiosk_cut_amount: u8,
        kiosk_cut_margin: u8,
        kiosk_present_type: u8,
        kiost_present_timeout: u16,
        presenter_loop_lenght: u16,
    },

    /// `^LF`, list fonts
    LF,

    /// `^LHx,y`, set label home position
    LH {
        /// 0 to 32000
        x: u32,
        /// 0 to 32000
        y: u32,
    },

    /// `^LLy.x`, set label length
    LL {
        /// 1 to 32000
        length: u32,
        /// applies to all media
        all_media: bool,
    },

    /// `^LRa`, label reverse print
    LR { reverse: bool },

    /// `^LSa`, label shift. Shift all field positions to the left
    LS {
        /// `-9999 to 9999`
        left_shift: Ranged<i16, -9999, 9999>,
    },

    /// `^LTx`, label top. Shift entire label format up or down
    LT {
        // range has restrictions that vary based on model. Large negative values can cause
        // media to unthread.
        /// Typical: -120 to 120
        label_top: Ranged<i16, -120, 120>,
    },

    ///  Set maintenance alerts
    ///
    /// `^MA<type>,<print>,<print label threshold>,<frequency>,<units>`
    MA {
        kind: AlertKind,
        print: bool,
        printlabel_threshold: AlertPrintlabelThreshold,
        /// 0 to 2000
        frequency: Ranged<u16, 0, 2000>,
    },

    /// Change bitmap clearing so that bitmap can be retained after printing
    ///
    /// `^MCa`, map clear.
    MC {
        /// default Y.
        map_clear: bool,
    },

    /// `^MDa`, media darkness
    MD { darkness_level: Ranged<i8, -30, 30> },

    /// `^MFp,h`, media feed
    MF {
        feed_action_on_power_up: FeedAction,
        feed_action_after_closing_printhead: FeedAction,
    },

    /// `^MI<type>,<message>`. Set maintenance information message
    MI {
        /// default `R`
        kind: AlertKind,
        /// Defaults:
        /// - `C`: "please clean printhead"
        /// - `R`: "please replace printhead"
        message: String,
    },

    /// `^MLa`
    ML {
        /// dpi * 2 to maximul length of label
        max_label_length: u16,
    },

    /// `^MMa,b`, print mode
    MM {
        print_mode: PrintMode,

        prepeel_select: bool,
    },

    /// `^MNa,b`, media tracking
    MN {
        media_kind: MediaKind,
        /// Default 0, various ranges dependent on model.
        black_mark_offset: i16,
    },

    /// `^MPa`, mode protection
    MP {
        /// - `D` = disable darkness mode
        /// - `P` = disable position mode
        /// - `C` = disable calibration mode
        /// - `E` = enable all modes
        /// - `S` = disable all mode saves
        /// - `W` = disable pause
        /// - `F` = disable feed
        /// - `X` = disable cancel
        /// - `M` = disable menu changes
        mode_and_enable: u8,
    },

    /// `^MTa`, media type
    MT {
        /// - `T` = thermal transfer media
        /// - `D` = direct thermal media
        media_type_used: u8,
    },

    /// `^MUa,b,c` set units of measurment
    ///
    /// Works field by field, once set carries over from field to field until a new mode of units
    /// is entered.
    MU {
        units: UnitKind,
        format_base_in_dots_per_inch: u16,
        desired_dots_per_inch_conversion: u16,
    },

    /// Modify head cold warning
    MW { enable_head_cold_warning: bool },

    /// `^NCa`, select the primary network device
    NC { primary_network_device: u8 },

    /// `~NC###`, network connect
    NCx { network_id: u16 },

    /// `^NDa,b,c,d,e,f,g,h,i,j`, change network settings
    ND {
        network_device: u8,
        ip_resolution: u8,
        ip_address: String,
        subnet_mask: String,
        default_gateway: String,
        wins_server: String,
        /// default true.
        connection_timeout_checking: bool,
        /// default 300.
        timeout_value: Ranged<u16, 0, 9999>,
        /// default 0 (no arp sent)
        arp_broadcast_interval: Ranged<u8, 0, 30>,
        /// default 9100
        base_raw_port_number: Ranged<u16, 1, 65535>,
    },

    /// `^NI###`, assing network id number
    NI { network_id: u16 },

    /// `~NR`
    NR,

    /// `^NSa,b,c,d,e,f,g,h,i`, change wired networking settings
    NS {
        ip_resolution: u8,
        ip_address: String,
        subnet_mask: String,
        default_gateway: String,
        wins_server: String,
        connection_timeout_checking: bool,
        timeout_value: Ranged<u16, 0, 9999>,
        arp_broadcast_interval: Ranged<u8, 0, 30>,
    },

    /// `~NT`, Set currently connected printer transparent
    ///
    /// RS-485 only
    NT,

    /// `^PAa,b,c,d`, Set Advanced Text Properties
    PA {
        /// 0 = off, 1 = on
        default_glyph: bool,

        bidirectional_text_layout: bool,

        character_shaping: bool,

        opentype_table_support: bool,
    },

    /// `^PF#`, Slew Given number of dot rows
    PF { slew: Ranged<u16, 0, 32000> },

    /// Feed one label after the format currently being printed is done or when the printer is
    /// placed in pause.
    /// `^PH`, `~PH`, slew to home position
    PH,

    /// `~PLa`, Present length addition
    PL { additional_eject_length_mm: u8 },

    /// `^PMa`, mirror the entire label
    PM { mirror: bool },

    /// `~PM`, decommissioning mode
    ///
    /// USB only. Exits protected mode.
    PMx {
        serial_number: String,
        number_of_flash_wipes: u8,
    },

    /// `^PNa`, present now
    ///
    /// Ejects 50mm + ~PL + ^PN.
    PN { media_eject_length_mm: u8 },

    /// `^POa`, rotate entire label 180 degrees.
    ///
    /// Inverts x,y coordinates.
    PO { rotate: bool },

    /// `^PP`/`~PP`, programmable pause.
    ///
    /// Pauses after the current label is complete
    ///
    /// `~PS` resumes.
    PP,

    /// `^PQq,p,r,o,e`, print quantity
    PQ {
        quantity: Ranged<u64, 1, 99_999_999>,
        pause: Ranged<u64, 0, 99_999_999>,
        replicas_of_each_serial: Ranged<u64, 1, 99_999_999>,
        override_pause_count: bool,
        cut_on_error_label: bool,
    },

    /// `~PR`, applicator reprint
    PR,

    /// `^PR`, print rate
    PRx {
        // enum, see docs
        print_speed: String,
    },

    /// `~PS`, print start
    PS,

    /// `^PWa`
    PW { label_width_dots: u16 },

    /// `~RO`, reset advanced counters
    RO { counter_number: u8 },

    /// `^SCa,b,c,d,e,f`, Set Serial Communications
    SC {
        baud_rate: u8,
        word_length: u8,
        parity: u8,
        stop_bits: u8,
        flow_control: SerialFlowControl,
        zebra_protocol: SerialZebraProtocol,
    },

    /// `~SD##`, set darkness
    SD { darkness: Ranged<u8, 0, 30> },

    /// `^SEd:o.x`
    SE {
        device: String,
        name: String,
        extension: String,
    },

    /// `^SFa,b`, Serialization Field (with a standard ^FD String)
    ///
    /// Firmware x.14 and later.
    SF {
        mask_string: String,
        increment_string: String,
    },

    /// `^SI`, set sensor intensity
    SI {
        sensor_setting: SensorSetting,
        intensity: Ranged<u8, 0, 196>,
    },

    /// `^SLa,b`, set mode and language for real time clock
    SL { mode: u8, language: u8 },

    /// `^SNv,n,z`, serialization data
    SN {
        starting_value: u32,
        change_value: u32,
        add_leading_zeros: bool,
    },

    /// `^SOa,b,c,d,e,f,g`, set offset for realtime clock
    SO {
        clock_set: u8,
        months_offset: i32,
        days_offset: i32,
        years_offset: i32,
        hours_offset: i32,
        minutes_offset: i32,
        seconds_offset: i32,
    },

    /// `^SPa`, start print
    SP { dot_row_to_start_printing: u32 },

    /// `^SQa,b,c`, halt zebranet alert
    SQ {
        condition_type: u8,
        destination: u8,
        halt_messages: bool,
    },

    /// `^SR####`, set printhead resistance
    SR { resistance: u16 },

    /// `^SSw,m,r,l,m2,r2,a,b,c`, set media sensors
    SS {
        web: Ranged<u8, 0, 100>,
        media: Ranged<u8, 0, 100>,
        ribbon: Ranged<u8, 0, 100>,
        label_length: Ranged<u32, 1, 32000>,
        intensity_of_media_led: Ranged<u8, 0, 255>,
        intensity_of_ribbon_led: Ranged<u8, 0, 255>,
        mark_sensing: Ranged<u8, 0, 100>,
        mark_media_sensing: Ranged<u8, 0, 100>,
        mark_led_sensing: Ranged<u8, 0, 255>,
    },

    /// `^STa,b,c,d,e,f,g`, set date and time for realtime clock
    ST {
        month: u8,
        day: u8,
        year: u16,
        hour: u8,
        minute: u8,
        second: u8,
        format: u8,
    },

    /// `^SXa,b,c,d,e,f`, set zebranet alert
    SX {
        condition_type: u8,
        destination_route_for_alert: u8,
        enable_condition_clear_alert: bool,
        destination_setting: String,
        port_number: u16,
    },

    /// `^SZa`, set ZPL mode
    SZ { zpl_mode: u8 },

    /// `~TA###`, tear-off adjust position
    TA {
        // smaller on
        adjust_position: Ranged<i8, -120, 120>,
    },

    /// `^TBa,b,c`, text blocks
    ///
    /// Firmware .14 and later
    TB {
        block_rotation: u8,
        block_width_in_dots: u16,
        block_height_in_dots: u16,
    },

    /// `^TOs:o.x,d:o.x`, transfer object
    TO {
        source_device: String,
        source_name: String,
        source_extension: String,
        destination_device: String,
        destination_name: String,
        destination_extension: String,
    },

    /// `~WC`, print configuration label
    WC,

    /// `^WDd:o.x` print directory label
    ///
    WD {
        device: String,
        name: String,
        extension: String,
    },

    /// `~WQ`, write query
    WQ { query_type: String },

    /// `^XA` start format
    XA,

    /// `^XB` suppress backfeed
    XB,

    /// `^XFd:o.x`, recall format
    XF,

    /// `^XGd:o.x,mx,my`, recall graphic
    XG {
        device: String,
        name: String,
        extension: String,
        magnify_x: Ranged<u8, 1, 10>,
        magnify_y: Ranged<u8, 1, 10>,
    },

    /// `^XS<length>,<threshold>`, set dynamic media calibration
    XS {
        length: bool,
        threshold: bool,
        gain: bool,
    },

    /// `^XZ`, end format
    XZ,

    /// `^ZZt,b`, printer sleep
    ZZ {
        sleep_timeout: u32,
        sleep_even_if_labels_pending: bool,
    },
    // `~EG`, see `^ID`
}

impl Command {
    fn is_rs485_only(&self) -> bool {
        // ~NC, ^NI, ~NR, ~NT
        match self {
            Command::NT => true,
            _ => false,
        }
    }
}

pub struct Ranged<T, const MIN: i128, const MAX: i128> {
    value: T,
}

impl<T, const MIN: i128, const MAX: i128> Deref for Ranged<T, MIN, MAX> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<T, const MIN: i128, const MAX: i128> DerefMut for Ranged<T, MIN, MAX> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.value
    }
}

pub enum BlackOrWhite {
    Black,
    White,
}

pub enum AlertKind {
    /// `R`
    HeadReplacement,
    /// `C`
    HeadCleaning,
}

pub enum AlertPrintlabelThreshold {
    /// `0`
    Off,
    /// `R`
    HeadReplacement,
    /// `C`
    HeadCleaning,
}

pub enum FeedAction {
    /// default, `~CL`
    C,
    ToFirstWebAfterSensor,
    /// `~JL`
    L,
    NoFeed,
    ShortCalibration,
}

/// Used in `^MM` command
pub enum PrintMode {
    /// `T`
    TearOff,
    /// `P`
    PeelOff,
    /// `R`
    Rewind,
    /// `A`
    Applicator,
    /// `C`
    Cutter,
    /// `D`
    DelayedCutter,
    /// `F`
    Rfid,
    /// `K`
    Kiosk,
    // L & U marked as reserved. RFID has some variation for R110PAX4 vs others.
    // Many of the values above are only supported on specific models.
}

pub enum MediaKind {
    /// `N`
    Continuous,
    /// `Y` and `W`
    NoncontinuousWebSensing,
    /// `M`
    NoncontinuousMarkSensing,
    /// `A`
    AutoDetectDurringCalibration,
    /// `V`
    ContinuousVariableLength,
}

/// Used in `^MU` command
pub enum UnitKind {
    Dots,
    Inches,
    Millimeters,
}

pub enum SerialFlowControl {
    /// `X`
    XonXoff,
    /// `D`
    DtrDsr,
    /// `R`
    Rts,
    /// `M`
    DtrDsrXonXoff,
}

pub enum SerialZebraProtocol {
    /// `A`
    AckNack,
    /// `N`
    None,
    /// `Z`
    Zebra,
}

/// Used in `^SI` command
pub enum SensorSetting {
    Brightness,
    Baseline,
}
