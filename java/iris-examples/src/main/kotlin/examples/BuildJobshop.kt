package examples

import dev.gecode.iris.IrisException
import dev.gecode.iris.Params
import dev.gecode.iris.build.Gecode
import dev.gecode.iris.build.IntField
import dev.gecode.iris.build.StringField
import dev.gecode.iris.kotlin.build.IrisSpec
import dev.gecode.iris.kotlin.build.atLeast
import dev.gecode.iris.kotlin.build.distinct
import dev.gecode.iris.kotlin.build.irisSpec
import dev.gecode.iris.kotlin.build.le
import dev.gecode.iris.kotlin.build.range
import dev.gecode.iris.kotlin.build.recordName
import dev.gecode.iris.kotlin.build.sum
import dev.gecode.iris.kotlin.build.word
import java.nio.file.Files
import java.nio.file.Path

private const val TWICE = "a job uses a machine twice; each job must use every machine once"

/** The handles of a job shop instance, which the builder returns beside the spec. */
class Shop(
    val name: StringField<String?>,
    val nJobs: IntField<Long>,
    val nMachines: IntField<Long>,
    val machine: IntField<List<LongArray>>,
    val duration: IntField<List<LongArray>>,
)

/**
 * Reads OR-Library and JSPLIB files: a header line with the numbers of jobs and machines, then one
 * line per job of machine and duration pairs.
 */
val jobshop: IrisSpec<Shop> =
    irisSpec("jobshop") {
        comment("#", "+++")
        instances {
            val name by optionalCapture(word())
            val nJobs by capture(atLeast(1), name = "n_jobs")
            val nMachines by capture(atLeast(1), name = "n_machines")
            // Error messages name the records of the jobs.
            val jobs = recordName("job", "jobs")
            val (machine, duration) =
                sections(skipOther = true) {
                    arm("instance ", name, optional = true)
                    arm(nJobs, " ", nMachines, last = true) {
                        check(
                            nJobs le (Gecode.intMax - 2) / 2 / nMachines,
                            "there are too many jobs and machines",
                        )
                        repeat(nJobs, jobs) {
                            repeat(nMachines) {
                                read("machine", range(0).below(nMachines)) to
                                    read("duration", atLeast(0))
                            }
                        }
                    }
                }
            check(distinct(jobs[machine]), TWICE)
            check(
                sum(jobs) { sum(it[duration]) } le Gecode.intMax,
                "the total duration is too large",
            )
            Shop(name, nJobs, nMachines, machine, duration)
        }
    }

fun main() {
    val (spec, shop) = jobshop
    val orLibrary = Path.of("formats/jobshop/cases/or-library/input.txt")
    val data = spec.read(orLibrary, Params.of("instance", "handmade-or"))
    println(
        "${data[shop.name] ?: "unnamed"}: ${data[shop.nJobs]} jobs on " +
            "${data[shop.nMachines]} machines"
    )
    val durations = data[shop.duration]
    for ((j, machines) in data[shop.machine].withIndex()) {
        println(
            "job $j: machines ${machines.contentToString()}, " +
                "durations ${durations[j].contentToString()}"
        )
    }

    // Errors name a path's input with the platform's separators, so this read
    // names the input itself to print the same on every platform.
    val invalid = "formats/jobshop/cases/duplicate-machine/input.txt"
    try {
        spec.read(
            Files.readAllBytes(Path.of(invalid)),
            invalid,
            Params.of("instance", "handmade-invalid"),
        )
    } catch (error: IrisException) {
        println()
        println("${error.code().code()} at ${error.origin()}:${error.line()}:${error.column()}")
        println(error.error().message())
        // The statement's call site: the line of the distinct check above.
        val site = error.callSite().orElseThrow()
        println("call site: ${site.fileName}:${site.lineNumber}")
    }
}
