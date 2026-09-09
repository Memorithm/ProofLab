import Mathlib.Data.Fin.VecNotation

namespace ProofLab.Descriptive

/-- A purely relational vocabulary. Each relation symbol carries a fixed arity. -/
structure RelationalVocabulary where
  Relation : Type
  arity : Relation → Nat

/-- An `arity`-tuple over the canonical finite carrier `Fin size`. -/
abbrev Tuple (size arity : Nat) := Fin arity → Fin size

/-- A finite relational structure with an explicit finite carrier size. -/
structure FiniteStructure (σ : RelationalVocabulary) where
  size : Nat
  interprets : (r : σ.Relation) → Tuple size (σ.arity r) → Prop

/--
A finite relational structure equipped with an arbitrary total ordering.

The ordering is represented by a bijective rank map into `Fin size`. This keeps
vertex identity separate from order position while making every order comparison
reduce to the trusted linear order on `Fin size`.
-/
structure OrderedFiniteStructure (σ : RelationalVocabulary) where
  base : FiniteStructure σ
  rank : Equiv (Fin base.size) (Fin base.size)

namespace OrderedFiniteStructure

/-- Strict order induced by the structure's rank map. -/
def lt {σ : RelationalVocabulary} (M : OrderedFiniteStructure σ)
    (x y : Fin M.base.size) : Prop :=
  M.rank x < M.rank y

/-- The induced order is irreflexive. -/
theorem order_irrefl {σ : RelationalVocabulary} (M : OrderedFiniteStructure σ)
    (x : Fin M.base.size) : ¬ M.lt x x := by
  simp [lt]

/-- The induced order is transitive. -/
theorem order_trans {σ : RelationalVocabulary} (M : OrderedFiniteStructure σ)
    {x y z : Fin M.base.size} (hxy : M.lt x y) (hyz : M.lt y z) : M.lt x z := by
  exact lt_trans hxy hyz

/-- Any two carrier elements satisfy strict-order trichotomy. -/
theorem order_trichotomy {σ : RelationalVocabulary} (M : OrderedFiniteStructure σ)
    (x y : Fin M.base.size) : M.lt x y ∨ x = y ∨ M.lt y x := by
  rcases lt_trichotomy (M.rank x) (M.rank y) with hxy | hxy | hyx
  · exact Or.inl hxy
  · exact Or.inr (Or.inl (M.rank.injective hxy))
  · exact Or.inr (Or.inr hyx)

/-- The induced strict order is asymmetric. -/
theorem order_asymm {σ : RelationalVocabulary} (M : OrderedFiniteStructure σ)
    {x y : Fin M.base.size} (hxy : M.lt x y) : ¬ M.lt y x := by
  intro hyx
  exact order_irrefl M x (order_trans M hxy hyx)

end OrderedFiniteStructure

namespace Fixtures

/-- Minimal vocabulary used to exercise unary and binary relation semantics. -/
inductive DemoRelation
  | marked
  | edge
  deriving DecidableEq, Repr

/-- Unary `marked` plus binary `edge`. -/
abbrev demoVocabulary : RelationalVocabulary where
  Relation := DemoRelation
  arity
    | .marked => 1
    | .edge => 2

/-- A three-element structure with one marked element and a directed path `0 → 1 → 2`. -/
abbrev demoStructure : FiniteStructure demoVocabulary where
  size := 3
  interprets
    | .marked, tuple => tuple 0 = 1
    | .edge, tuple => (tuple 0 = 0 ∧ tuple 1 = 1) ∨ (tuple 0 = 1 ∧ tuple 1 = 2)

/-- The canonical identity ranking gives one concrete ordered expansion. -/
abbrev demoOrdered : OrderedFiniteStructure demoVocabulary where
  base := demoStructure
  rank := Equiv.refl _

example : demoStructure.interprets .marked (fun _ => 1) := by
  rfl

example : demoStructure.interprets .edge ![0, 1] := by
  simp [demoStructure]

example : demoOrdered.lt 0 2 := by
  decide

example : ¬ demoOrdered.lt 2 0 := by
  decide

end Fixtures

end ProofLab.Descriptive
