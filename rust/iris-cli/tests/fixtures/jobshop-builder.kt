import dev.gecode.iris.build.Gecode
import dev.gecode.iris.kotlin.build.IrisBuilder
import dev.gecode.iris.kotlin.build.atLeast
import dev.gecode.iris.kotlin.build.capture
import dev.gecode.iris.kotlin.build.distinct
import dev.gecode.iris.kotlin.build.dynamic
import dev.gecode.iris.kotlin.build.irisBuilder
import dev.gecode.iris.kotlin.build.le
import dev.gecode.iris.kotlin.build.over
import dev.gecode.iris.kotlin.build.range
import dev.gecode.iris.kotlin.build.recordName
import dev.gecode.iris.kotlin.build.skipOther
import dev.gecode.iris.kotlin.build.sum
import dev.gecode.iris.kotlin.build.word

private val name = dynamic("name")
private val nJobs = dynamic("n_jobs")
private val nMachines = dynamic("n_machines")
private val machine = dynamic("machine")
private val duration = dynamic("duration")
private val job = recordName("job", "jobs")

fun specBuilder(): IrisBuilder<Unit> =
    irisBuilder("jobshop") {
        comment("#", "+++")
        instances {
            sections(skipOther()) {
                arm("instance ", capture(name, word()), optional = true)
                arm(capture(nJobs, atLeast(1)), " ", capture(nMachines, atLeast(1)), last = true) {
                    check(
                        nJobs le (Gecode.intMax - 2) / 2 / nMachines,
                        "there are too many jobs and machines",
                    )
                    repeat(nJobs, job) {
                        repeat(nMachines, recordName("step", "steps")) {
                            read(machine, range(0).below(nMachines))
                            read(duration, atLeast(0))
                        }
                    }
                }
            }
            check(
                distinct(job[machine]),
                "a job uses a machine twice; each job must use every machine once",
            )
            check(
                sum(over(job.list()).named("j")) { j -> sum(j[duration]) } le Gecode.intMax,
                "the total duration is too large",
            )
        }
    }

fun main() {
    print(specBuilder().source())
}
