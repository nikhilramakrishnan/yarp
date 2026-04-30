use super::*;

/*
0 1 2 3
w a r p
-------
0     4  << the span for the string "yarp" is (0, 4)

Spanned {
    item: String::new("yarp"),  << yarp string
    span: Span::new(0, 4)       << span
}

or >> String::new("yarp").spanned(Span::new(0, 4))        */
fn yarp() -> Spanned<String> {
    String::from("yarp").spanned(Span::new(0, 4))
}

fn empty() -> Spanned<String> {
    String::new().spanned_unknown()
}

#[test]
fn knows_distances() {
    assert!(yarp().span.distance() == 4);
    assert!(empty().span.distance() == 0);
}
