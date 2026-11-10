//! Regression: movement completion must not destroy the piston base.
//! East-facing regular piston, adjacent floor lever, empty space ahead.
//! This exercises server simulation, not the integrated client or GPU.
use super::*;

fn snapshot(w: &World<'_>, label: &str, base: BlockPos, head: BlockPos) {
    let drops: Vec<_> = w
        .level
        .events
        .iter()
        .filter_map(|e| match e {
            LevelEvent::DropResources { pos, state } => Some((*pos, *state)),
            _ => None,
        })
        .collect();
    eprintln!(
        "[PHASE1] {label} time={} base={} front={} moving={:?} drops={drops:?}",
        w.level.time,
        w.name(base),
        w.name(head),
        w.ticks.moving_pistons
    );
    assert!(
        drops.is_empty(),
        "unexpected resource-drop event at {label}: {drops:?}"
    );
}

#[test]
fn player_breaking_moving_head_removes_extended_base() {
    let f = fixture().expect("valid prepared 26.3 resources required");
    for creative in [false, true] {
        let mut w = World::new(f);
        let base = BlockPos::new(0, 1, 0);
        let head = BlockPos::new(1, 1, 0);
        let extended = w.id("piston", &[("facing", "east"), ("extended", "true")]);
        let moving = w.id("moving_piston", &[("facing", "east"), ("type", "normal")]);
        w.put(base, extended);
        w.put(head, moving);
        assert!(
            w.b()
                .player_destroy_block(&mut w.level, &mut w.ticks, head, creative, true)
        );
        assert_eq!(w.get(base), w.b().air(), "creative={creative}");
    }
}

#[test]
fn both_piston_variants_complete_repeated_cycles_in_every_direction() {
    let f = fixture().expect("valid prepared 26.3 resources required");
    for kind in ["piston", "sticky_piston"] {
        for (facing, delta) in [
            ("east", (1, 0, 0)),
            ("west", (-1, 0, 0)),
            ("up", (0, 1, 0)),
            ("down", (0, -1, 0)),
            ("north", (0, 0, -1)),
            ("south", (0, 0, 1)),
        ] {
            let mut w = World::new(f);
            let base = BlockPos::new(0, 10, 0);
            let head = base.offset(delta.0, delta.1, delta.2);
            let power = base.offset(-delta.0, -delta.1, -delta.2);
            let piston = w.id(kind, &[("facing", facing), ("extended", "false")]);
            let redstone = w.id("redstone_block", &[]);
            let air = w.b().air();
            w.set(base, piston);
            for cycle in 0..3 {
                w.set(power, redstone);
                for _ in 0..5 {
                    w.full_tick();
                }
                assert_eq!(w.prop(base, "extended"), "true", "{kind}/{facing}/{cycle}");
                assert!(w.name(head).starts_with("piston_head["));
                assert!(w.ticks.moving_pistons.is_empty());
                w.set(power, air);
                for _ in 0..5 {
                    w.full_tick();
                }
                assert_eq!(w.get(base), piston, "{kind}/{facing}/{cycle}");
                assert_eq!(w.get(head), air);
                assert!(w.ticks.moving_pistons.is_empty());
            }
        }
    }
}

#[test]
fn regular_piston_direct_lever_preserves_base() {
    // Unlike world!(), missing resources MUST fail, never silently pass.
    let f = fixture().expect("PHASE1 BLOCKED: valid prepared 26.3 resources required");
    let mut w = World::new(f);
    let base = BlockPos::new(0, 1, 0);
    let head = BlockPos::new(1, 1, 0);
    let lever_pos = BlockPos::new(-1, 1, 0);
    w.floor(0, 2, "stone");
    let piston = w.id("piston", &[("facing", "east"), ("extended", "false")]);
    let extended = w.id("piston", &[("facing", "east"), ("extended", "true")]);
    let head_state = w.id(
        "piston_head",
        &[("facing", "east"), ("short", "false"), ("type", "normal")],
    );
    let moving = w.id("moving_piston", &[("facing", "east"), ("type", "normal")]);
    let lever = w.id(
        "lever",
        &[("face", "floor"), ("facing", "north"), ("powered", "false")],
    );
    let air = w.b().air();
    assert!(w.set(base, piston));
    assert!(w.set(lever_pos, lever));
    snapshot(&w, "before", base, head);
    assert_eq!(w.get(base), piston);
    assert_eq!(w.get(head), air);
    assert!(w.ticks.moving_pistons.is_empty());

    assert_eq!(w.use_block(lever_pos), InteractionResult::Success);
    assert_eq!(w.prop(lever_pos, "powered"), "true");
    snapshot(&w, "lever-on-before-events", base, head);
    w.b().run_block_events(&mut w.level, &mut w.ticks);
    snapshot(&w, "extension-start", base, head);
    assert_eq!(w.get(base), extended);
    assert_eq!(w.get(head), moving);
    assert_eq!(w.ticks.moving_pistons.len(), 1);
    let (pos, entity) = w.ticks.moving_pistons[0];
    assert_eq!(pos, head);
    assert_eq!(entity.moved_state, head_state);
    assert!(entity.extending && entity.source);
    assert_eq!(entity.direction, Direction::East);
    assert_eq!(entity.progress, 0.0);
    for tick in 1..=4 {
        w.full_tick();
        snapshot(&w, &format!("extension-tick-{tick}"), base, head);
        assert_eq!(w.get(base), extended, "base lost during extension");
        if tick <= 2 {
            assert_eq!(w.get(head), moving);
            assert_eq!(w.ticks.moving_pistons.len(), 1);
            assert_eq!(w.ticks.moving_pistons[0].1.progress, tick as f32 * 0.5);
        }
    }
    assert_eq!(w.get(head), head_state);
    assert!(w.ticks.moving_pistons.is_empty());

    assert_eq!(w.use_block(lever_pos), InteractionResult::Success);
    assert_eq!(w.prop(lever_pos, "powered"), "false");
    w.b().run_block_events(&mut w.level, &mut w.ticks);
    snapshot(&w, "retraction-start", base, head);
    assert_eq!(w.get(base), moving);
    assert_eq!(w.get(head), air);
    assert_eq!(w.ticks.moving_pistons.len(), 1);
    let (pos, entity) = w.ticks.moving_pistons[0];
    assert_eq!(pos, base);
    assert_eq!(entity.moved_state, piston);
    assert!(!entity.extending && entity.source);
    assert_eq!(entity.direction, Direction::East);
    for tick in 1..=4 {
        w.full_tick();
        snapshot(&w, &format!("retraction-tick-{tick}"), base, head);
        assert_eq!(w.get(head), air);
        if tick <= 2 {
            assert_eq!(w.get(base), moving);
            assert_eq!(w.ticks.moving_pistons.len(), 1);
            assert_eq!(w.ticks.moving_pistons[0].1.moved_state, piston);
        } else {
            assert_eq!(w.get(base), piston, "base not restored after retraction");
            assert!(w.ticks.moving_pistons.is_empty());
        }
    }
}
