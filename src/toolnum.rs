//! Tool numbers (`T#`) and length offsets (`H#`) of the tools a program uses (phase 3 of `docs/post-processor-spec.md`).
//!
//! The slot belongs to the machine, not the tool: the same cutter can be slot 3 on one machine and slot 7 on another. A tool's number
//! is resolved at generate time, first match wins:
//!
//! 1. the machine's tool table (`Machine.tool_table`) entry for the tool's id (the generator reads the project's copy of the machine,
//!    `ProjectData.machine`);
//! 2. the tool's own number (`Tool.number`);
//! 3. the order of first use in the program, only when the post says `tool_numbers = "first_use"`. These numbers skip every number
//!    already taken by a tool resolved in steps 1 and 2 *in this program*, so a numbered tool used later never shares its number.
//!
//! With `tool_numbers = "required"` a used tool that resolves to step 3 is an error naming each such tool. Two different used tools
//! on one number are an error with every post. The length offset is the table entry's own (when the number came from the table), else
//! the tool's (`Tool.length_offset`), else the resolved number (`H5` for `T5`, the Fanuc / Mach convention).
use crate::i18n::tr_f;
use crate::model::{Machine, Tool};
use crate::post::ToolNumbers;
use crate::program::NumberFrom;

/// A tool's resolved number, length offset and where the number came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Resolved { pub number: u16, pub offset: u16, pub from: NumberFrom }

fn label(t: &Tool) -> &str { if t.name.trim().is_empty() { &t.id } else { &t.name } }

/// The numbers of `used` (the tools of one program, in order of first use, each once) on `machine`, for a post whose
/// `caps.tool_numbers` is `mode`. The error is a sentence for the export notes.
pub fn resolve(used: &[&Tool], machine: &Machine, mode: ToolNumbers) -> Result<Vec<Resolved>, String> {
    // steps 1 and 2
    let fixed: Vec<Option<Resolved>> = used.iter().map(|t| {
        if let Some(s) = machine.tool_table.iter().find(|s| s.tool_id == t.id) {
            return Some(Resolved { number: s.number, offset: s.length_offset.unwrap_or(s.number), from: NumberFrom::Rack });
        }
        t.number.map(|n| Resolved { number: n, offset: t.length_offset.unwrap_or(n), from: NumberFrom::Tool })
    }).collect();
    if mode == ToolNumbers::Required {
        let missing: Vec<&str> = used.iter().zip(&fixed).filter(|(_, r)| r.is_none()).map(|(t, _)| label(t)).collect();
        if !missing.is_empty() {
            return Err(tr_f("This post changes tools itself, so every tool needs its rack slot, and these have none: {tools}. Put them in the machine's tool rack (or give the tool a number in the tool manager).",
                            &[("tools", &missing.join(", "))]));
        }
    }
    // step 3: order of first use, skipping the numbers taken above
    let taken: std::collections::BTreeSet<u16> = fixed.iter().flatten().map(|r| r.number).collect();
    let mut next: u16 = 1;
    let out: Vec<Resolved> = used.iter().zip(fixed).map(|(t, r)| r.unwrap_or_else(|| {
        while taken.contains(&next) { next = next.saturating_add(1); }
        let n = next;
        next = next.saturating_add(1);
        Resolved { number: n, offset: t.length_offset.unwrap_or(n), from: NumberFrom::FirstUse }
    })).collect();
    // two different tools on one number: the wrong cutter would be loaded
    let mut clash: Vec<String> = Vec::new();
    for (i, a) in out.iter().enumerate() {
        if out[..i].iter().any(|b| b.number == a.number) { continue; }
        let same: Vec<&str> = out.iter().zip(used).filter(|(b, _)| b.number == a.number).map(|(_, t)| label(t)).collect();
        if same.len() > 1 { clash.push(format!("T{} ({})", a.number, same.join(", "))); }
    }
    if !clash.is_empty() {
        return Err(tr_f("Two tools have the same tool number, so the machine would load the wrong one: {clash}. Give each tool its own slot in the machine's tool rack.", &[("clash", &clash.join("; "))]));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Tool, ToolSlot};

    fn tool(id: &str, number: Option<u16>, off: Option<u16>) -> Tool {
        let mut t = crate::setup::starter_tools(Default::default()).remove(0);
        t.id = id.into(); t.name = format!("tool {id}"); t.number = number; t.length_offset = off;
        t
    }
    fn rack(slots: &[(&str, u16, Option<u16>)]) -> Machine {
        Machine { tool_table: slots.iter().map(|&(id, n, h)| ToolSlot { tool_id: id.into(), number: n, length_offset: h }).collect(), ..Machine::default() }
    }
    fn nums(r: &[Resolved]) -> Vec<(u16, u16, NumberFrom)> { r.iter().map(|r| (r.number, r.offset, r.from)).collect() }
    use NumberFrom::*;

    #[test]
    fn the_table_wins_over_the_tool_which_wins_over_first_use() {
        let (a, b, c) = (tool("a", Some(3), None), tool("b", Some(4), None), tool("c", None, None));
        let m = rack(&[("a", 7, None), ("zz", 9, None)]);
        let r = resolve(&[&a, &b, &c], &m, ToolNumbers::FirstUse).unwrap();
        assert_eq!(nums(&r), vec![(7, 7, Rack), (4, 4, Tool), (1, 1, FirstUse)]);
        // the first matching slot of the table is the one
        let m2 = rack(&[("a", 5, None), ("a", 6, None)]);
        assert_eq!(resolve(&[&a], &m2, ToolNumbers::Required).unwrap()[0].number, 5);
    }

    #[test]
    fn without_any_number_it_is_the_order_of_first_use() {
        let (a, b, c) = (tool("a", None, None), tool("b", None, None), tool("c", None, None));
        let r = resolve(&[&b, &a, &c], &Machine::default(), ToolNumbers::FirstUse).unwrap();
        assert_eq!(nums(&r), vec![(1, 1, FirstUse), (2, 2, FirstUse), (3, 3, FirstUse)]);
    }

    #[test]
    fn first_use_numbers_skip_the_numbers_taken_in_this_program() {
        // b (used second) has number 1, so a (used first) gets 2 and c 3; d's slot 4 is skipped too
        let (a, b, c, d, e) = (tool("a", None, None), tool("b", Some(1), None), tool("c", None, None), tool("d", None, None), tool("e", None, None));
        let m = rack(&[("d", 4, None), ("unused", 2, None)]);
        let r = resolve(&[&a, &b, &c, &d, &e], &m, ToolNumbers::FirstUse).unwrap();
        assert_eq!(r.iter().map(|r| r.number).collect::<Vec<_>>(), vec![2, 1, 3, 4, 5]);
        // a slot of a tool this program does not use is not taken
        let r = resolve(&[&a], &m, ToolNumbers::FirstUse).unwrap();
        assert_eq!(r[0].number, 1);
    }

    #[test]
    fn the_length_offset_defaults_to_the_number_and_can_be_set() {
        let (a, b, c, d) = (tool("a", None, Some(21)), tool("b", Some(8), Some(18)), tool("c", Some(9), Some(19)), tool("d", None, None));
        // a's slot has no offset of its own: the slot number, not the tool's 21 (that goes with the tool's own number)
        let m = rack(&[("a", 5, None), ("c", 6, Some(16))]);
        let r = resolve(&[&a, &b, &c, &d], &m, ToolNumbers::FirstUse).unwrap();
        assert_eq!(nums(&r), vec![(5, 5, Rack), (8, 18, Tool), (6, 16, Rack), (1, 1, FirstUse)]);
        // a first-use tool keeps its own offset
        let r = resolve(&[&tool("e", None, Some(30))], &Machine::default(), ToolNumbers::FirstUse).unwrap();
        assert_eq!(nums(&r), vec![(1, 30, FirstUse)]);
    }

    #[test]
    fn required_numbers_name_every_tool_without_one() {
        let (a, b, c) = (tool("a", None, None), tool("b", Some(2), None), tool("c", None, None));
        let e = resolve(&[&a, &b, &c], &rack(&[("x", 1, None)]), ToolNumbers::Required).unwrap_err();
        assert!(e.contains("tool a, tool c") && !e.contains("tool b"), "{e}");
        assert!(resolve(&[&b], &Machine::default(), ToolNumbers::Required).is_ok());
    }

    #[test]
    fn two_tools_on_one_number_is_an_error_with_every_post() {
        let (a, b, c) = (tool("a", None, None), tool("b", Some(5), None), tool("c", None, None));
        let m = rack(&[("a", 5, None), ("c", 7, None)]);
        for mode in [ToolNumbers::FirstUse, ToolNumbers::Required] {
            let e = resolve(&[&a, &b, &c], &m, mode).unwrap_err();
            assert!(e.contains("T5 (tool a, tool b)") && !e.contains("T7"), "{e}");
        }
        // the same tool used again is not a clash (each tool comes once), and two slots of unused tools never clash
        assert!(resolve(&[&a, &c], &rack(&[("a", 5, None), ("c", 7, None), ("x", 7, None)]), ToolNumbers::Required).is_ok());
    }
}
