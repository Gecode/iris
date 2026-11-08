package examples

import dev.gecode.iris.build.Gecode
import dev.gecode.iris.build.IntField
import dev.gecode.iris.kotlin.build.IrisSpec
import dev.gecode.iris.kotlin.build.atLeast
import dev.gecode.iris.kotlin.build.int2
import dev.gecode.iris.kotlin.build.irisSpec
import dev.gecode.iris.kotlin.build.rangeTo
import dev.gecode.iris.kotlin.build.unaryMinus
import java.nio.file.Path

/** The literal that ends a clause. */
private const val END_OF_CLAUSE = 0L

/** The handles of a CNF formula. */
class Cnf(
    val vars: IntField<Long>,
    val clauses: IntField<Long>,
    val clause: IntField<List<LongArray>>,
)

/** A DIMACS CNF reader: the number of variables and one row of literals per clause. */
val dimacsCnf: IrisSpec<Cnf> =
    irisSpec("dimacs-cnf") {
        comment("c")
        stop("%") // SATLIB files end with "%" and "0"
        val vars by capture(0..Gecode.intMax)
        val clauses by capture(atLeast(0))
        line("p cnf ", vars, " ", clauses)
        val clause = int2("clause")
        // The terminating 0 is not stored.
        repeat(clauses) { list(clause, (-vars..vars).until(END_OF_CLAUSE)) }
        Cnf(vars, clauses, clause)
    }

fun main() {
    val (spec, cnf) = dimacsCnf
    val data = spec.read(Path.of("formats/dimacs-cnf/cases/three-variables/input.txt"))
    println("${data[cnf.vars]} variables")
    for (literals in data[cnf.clause]) {
        println(literals.contentToString())
    }
}
