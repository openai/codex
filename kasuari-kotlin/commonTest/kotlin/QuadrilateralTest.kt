package kasuari

import kotlin.test.Test
import kotlin.test.assertEquals

class QuadrilateralTest {

    data class Point(
        val x: Variable,
        val y: Variable
    ) {
        companion object {
            fun new(): Point = Point(Variable.new(), Variable.new())
        }
    }

    @Test
    fun testQuadrilateral() {
        val (valueOf, updateValues) = newValues()

        val points = listOf(Point.new(), Point.new(), Point.new(), Point.new())
        val pointStarts = listOf(
            Pair(10.0, 10.0),
            Pair(10.0, 200.0),
            Pair(200.0, 200.0),
            Pair(200.0, 10.0)
        )
        val midpoints = listOf(Point.new(), Point.new(), Point.new(), Point.new())
        val solver = Solver.new()
        var weight = 1.0
        val multiplier = 2.0

        for (i in 0 until 4) {
            solver.addConstraints(listOf(
                points[i].x with WeightedRelation.EQ(Strength.WEAK * weight) to pointStarts[i].first,
                points[i].y with WeightedRelation.EQ(Strength.WEAK * weight) to pointStarts[i].second
            ))
            weight *= multiplier
        }

        val edges = listOf(Pair(0, 1), Pair(1, 2), Pair(2, 3), Pair(3, 0))
        for ((start, end) in edges) {
            solver.addConstraints(listOf(
                midpoints[start].x with WeightedRelation.EQ(Strength.REQUIRED) to ((points[start].x + points[end].x) / 2.0),
                midpoints[start].y with WeightedRelation.EQ(Strength.REQUIRED) to ((points[start].y + points[end].y) / 2.0)
            ))
        }

        solver.addConstraints(listOf(
            (points[0].x + 20.0) with WeightedRelation.LE(Strength.STRONG) to points[2].x,
            (points[0].x + 20.0) with WeightedRelation.LE(Strength.STRONG) to points[3].x,
            (points[1].x + 20.0) with WeightedRelation.LE(Strength.STRONG) to points[2].x,
            (points[1].x + 20.0) with WeightedRelation.LE(Strength.STRONG) to points[3].x,
            (points[0].y + 20.0) with WeightedRelation.LE(Strength.STRONG) to points[1].y,
            (points[0].y + 20.0) with WeightedRelation.LE(Strength.STRONG) to points[2].y,
            (points[3].y + 20.0) with WeightedRelation.LE(Strength.STRONG) to points[1].y,
            (points[3].y + 20.0) with WeightedRelation.LE(Strength.STRONG) to points[2].y
        ))

        for (point in points) {
            solver.addConstraints(listOf(
                point.x with WeightedRelation.GE(Strength.REQUIRED) to 0.0,
                point.y with WeightedRelation.GE(Strength.REQUIRED) to 0.0,
                point.x with WeightedRelation.LE(Strength.REQUIRED) to 500.0,
                point.y with WeightedRelation.LE(Strength.REQUIRED) to 500.0
            ))
        }

        updateValues(solver.fetchChanges())

        assertEquals(
            listOf(
                Pair(valueOf(midpoints[0].x), valueOf(midpoints[0].y)),
                Pair(valueOf(midpoints[1].x), valueOf(midpoints[1].y)),
                Pair(valueOf(midpoints[2].x), valueOf(midpoints[2].y)),
                Pair(valueOf(midpoints[3].x), valueOf(midpoints[3].y))
            ),
            listOf(
                Pair(10.0, 105.0),
                Pair(105.0, 200.0),
                Pair(200.0, 105.0),
                Pair(105.0, 10.0)
            )
        )

        solver.addEditVariable(points[2].x, Strength.STRONG)
        solver.addEditVariable(points[2].y, Strength.STRONG)
        solver.suggestValue(points[2].x, 300.0)
        solver.suggestValue(points[2].y, 400.0)

        updateValues(solver.fetchChanges())

        assertEquals(
            listOf(
                Pair(valueOf(points[0].x), valueOf(points[0].y)),
                Pair(valueOf(points[1].x), valueOf(points[1].y)),
                Pair(valueOf(points[2].x), valueOf(points[2].y)),
                Pair(valueOf(points[3].x), valueOf(points[3].y))
            ),
            listOf(
                Pair(10.0, 10.0),
                Pair(10.0, 200.0),
                Pair(300.0, 400.0),
                Pair(200.0, 10.0)
            )
        )

        assertEquals(
            listOf(
                Pair(valueOf(midpoints[0].x), valueOf(midpoints[0].y)),
                Pair(valueOf(midpoints[1].x), valueOf(midpoints[1].y)),
                Pair(valueOf(midpoints[2].x), valueOf(midpoints[2].y)),
                Pair(valueOf(midpoints[3].x), valueOf(midpoints[3].y))
            ),
            listOf(
                Pair(10.0, 105.0),
                Pair(155.0, 300.0),
                Pair(250.0, 205.0),
                Pair(105.0, 10.0)
            )
        )
    }
}
