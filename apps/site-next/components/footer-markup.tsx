// Ported from apps/site/partials/footer.html by scripts/convert.py.
import Link from 'next/link'

export function FooterMarkup() {
  return (
    <>
      <footer className="foot reveal-foot">
        {' '}
        <div className="foot-horizon" aria-hidden="true">
          {' '}
          <svg
            className="horizon"
            viewBox="0 0 1200 340"
            preserveAspectRatio="xMidYMax meet"
            focusable="false"
          >
            {' '}
            <path className="orbit" d="M 160 336 A 440 440 0 0 1 1040 336" />{' '}
            <path className="arc" pathLength="1" d="M 352 336 A 248 248 0 0 1 848 336" />{' '}
            <g className="ticks">
              {' '}
              <line
                className="t tide"
                x1="338.1"
                y1="329.6"
                x2="304.1"
                y2="328.7"
                style={{ '--i': '0' }}
              />{' '}
              <line
                className="t iris"
                x1="338.7"
                y1="316.7"
                x2="304.8"
                y2="314.2"
                style={{ '--i': '1' }}
              />{' '}
              <line
                className="t tide"
                x1="340"
                y1="303.9"
                x2="306.2"
                y2="299.8"
                style={{ '--i': '2' }}
              />{' '}
              <line
                className="t volt"
                x1="341.9"
                y1="291.2"
                x2="308.4"
                y2="285.4"
                style={{ '--i': '3' }}
              />{' '}
              <line
                className="t iris"
                x1="344.4"
                y1="278.6"
                x2="311.2"
                y2="271.1"
                style={{ '--i': '4' }}
              />{' '}
              <line
                className="t tide"
                x1="347.5"
                y1="266.1"
                x2="314.7"
                y2="257.1"
                style={{ '--i': '5' }}
              />{' '}
              <line
                className="t iris"
                x1="351.2"
                y1="253.8"
                x2="318.9"
                y2="243.2"
                style={{ '--i': '6' }}
              />{' '}
              <line
                className="t tide"
                x1="355.6"
                y1="241.7"
                x2="323.8"
                y2="229.5"
                style={{ '--i': '7' }}
              />{' '}
              <line
                className="t iris"
                x1="360.5"
                y1="229.8"
                x2="329.4"
                y2="216"
                style={{ '--i': '8' }}
              />{' '}
              <line
                className="t volt"
                x1="366"
                y1="218.2"
                x2="335.6"
                y2="202.9"
                style={{ '--i': '9' }}
              />{' '}
              <line
                className="t tide"
                x1="372"
                y1="206.9"
                x2="342.5"
                y2="190.1"
                style={{ '--i': '10' }}
              />{' '}
              <line
                className="t iris"
                x1="378.6"
                y1="195.8"
                x2="349.9"
                y2="177.6"
                style={{ '--i': '11' }}
              />{' '}
              <line
                className="t tide"
                x1="385.8"
                y1="185.1"
                x2="358"
                y2="165.6"
                style={{ '--i': '12' }}
              />{' '}
              <line
                className="t iris"
                x1="393.5"
                y1="174.8"
                x2="366.6"
                y2="153.9"
                style={{ '--i': '13' }}
              />{' '}
              <line
                className="t tide"
                x1="401.6"
                y1="164.9"
                x2="375.9"
                y2="142.7"
                style={{ '--i': '14' }}
              />{' '}
              <line
                className="t volt"
                x1="410.2"
                y1="155.3"
                x2="385.6"
                y2="131.9"
                style={{ '--i': '15' }}
              />{' '}
              <line
                className="t iris"
                x1="419.3"
                y1="146.2"
                x2="395.9"
                y2="121.6"
                style={{ '--i': '16' }}
              />{' '}
              <line
                className="t tide"
                x1="428.9"
                y1="137.6"
                x2="406.7"
                y2="111.9"
                style={{ '--i': '17' }}
              />{' '}
              <line
                className="t iris"
                x1="438.8"
                y1="129.5"
                x2="417.9"
                y2="102.6"
                style={{ '--i': '18' }}
              />{' '}
              <line
                className="t tide"
                x1="449.1"
                y1="121.8"
                x2="429.6"
                y2="94"
                style={{ '--i': '19' }}
              />{' '}
              <line
                className="t iris"
                x1="459.8"
                y1="114.6"
                x2="441.6"
                y2="85.9"
                style={{ '--i': '20' }}
              />{' '}
              <line
                className="t volt"
                x1="470.9"
                y1="108"
                x2="454.1"
                y2="78.5"
                style={{ '--i': '21' }}
              />{' '}
              <line
                className="t tide"
                x1="482.2"
                y1="102"
                x2="466.9"
                y2="71.6"
                style={{ '--i': '22' }}
              />{' '}
              <line
                className="t iris"
                x1="493.8"
                y1="96.5"
                x2="480"
                y2="65.4"
                style={{ '--i': '23' }}
              />{' '}
              <line
                className="t tide"
                x1="505.7"
                y1="91.6"
                x2="493.5"
                y2="59.8"
                style={{ '--i': '24' }}
              />{' '}
              <line
                className="t iris"
                x1="517.8"
                y1="87.2"
                x2="507.2"
                y2="54.9"
                style={{ '--i': '25' }}
              />{' '}
              <line
                className="t tide"
                x1="530.1"
                y1="83.5"
                x2="521.1"
                y2="50.7"
                style={{ '--i': '26' }}
              />{' '}
              <line
                className="t volt"
                x1="542.6"
                y1="80.4"
                x2="535.1"
                y2="47.2"
                style={{ '--i': '27' }}
              />{' '}
              <line
                className="t iris"
                x1="555.2"
                y1="77.9"
                x2="549.4"
                y2="44.4"
                style={{ '--i': '28' }}
              />{' '}
              <line
                className="t tide"
                x1="567.9"
                y1="76"
                x2="563.8"
                y2="42.2"
                style={{ '--i': '29' }}
              />{' '}
              <line
                className="t iris"
                x1="580.7"
                y1="74.7"
                x2="578.2"
                y2="40.8"
                style={{ '--i': '30' }}
              />{' '}
              <line
                className="t tide"
                x1="593.6"
                y1="74.1"
                x2="592.7"
                y2="40.1"
                style={{ '--i': '31' }}
              />{' '}
              <line
                className="t iris"
                x1="606.4"
                y1="74.1"
                x2="607.3"
                y2="40.1"
                style={{ '--i': '32' }}
              />{' '}
              <line
                className="t volt"
                x1="619.3"
                y1="74.7"
                x2="621.8"
                y2="40.8"
                style={{ '--i': '33' }}
              />{' '}
              <line
                className="t tide"
                x1="632.1"
                y1="76"
                x2="636.2"
                y2="42.2"
                style={{ '--i': '34' }}
              />{' '}
              <line
                className="t iris"
                x1="644.8"
                y1="77.9"
                x2="650.6"
                y2="44.4"
                style={{ '--i': '35' }}
              />{' '}
              <line
                className="t tide"
                x1="657.4"
                y1="80.4"
                x2="664.9"
                y2="47.2"
                style={{ '--i': '36' }}
              />{' '}
              <line
                className="t iris"
                x1="669.9"
                y1="83.5"
                x2="678.9"
                y2="50.7"
                style={{ '--i': '37' }}
              />{' '}
              <line
                className="t tide"
                x1="682.2"
                y1="87.2"
                x2="692.8"
                y2="54.9"
                style={{ '--i': '38' }}
              />{' '}
              <line
                className="t volt"
                x1="694.3"
                y1="91.6"
                x2="706.5"
                y2="59.8"
                style={{ '--i': '39' }}
              />{' '}
              <line
                className="t iris"
                x1="706.2"
                y1="96.5"
                x2="720"
                y2="65.4"
                style={{ '--i': '40' }}
              />{' '}
              <line
                className="t tide"
                x1="717.8"
                y1="102"
                x2="733.1"
                y2="71.6"
                style={{ '--i': '41' }}
              />{' '}
              <line
                className="t iris"
                x1="729.1"
                y1="108"
                x2="745.9"
                y2="78.5"
                style={{ '--i': '42' }}
              />{' '}
              <line
                className="t tide"
                x1="740.2"
                y1="114.6"
                x2="758.4"
                y2="85.9"
                style={{ '--i': '43' }}
              />{' '}
              <line
                className="t iris"
                x1="750.9"
                y1="121.8"
                x2="770.4"
                y2="94"
                style={{ '--i': '44' }}
              />{' '}
              <line
                className="t volt"
                x1="761.2"
                y1="129.5"
                x2="782.1"
                y2="102.6"
                style={{ '--i': '45' }}
              />{' '}
              <line
                className="t tide"
                x1="771.1"
                y1="137.6"
                x2="793.3"
                y2="111.9"
                style={{ '--i': '46' }}
              />{' '}
              <line
                className="t iris"
                x1="780.7"
                y1="146.2"
                x2="804.1"
                y2="121.6"
                style={{ '--i': '47' }}
              />{' '}
              <line
                className="t tide"
                x1="789.8"
                y1="155.3"
                x2="814.4"
                y2="131.9"
                style={{ '--i': '48' }}
              />{' '}
              <line
                className="t iris"
                x1="798.4"
                y1="164.9"
                x2="824.1"
                y2="142.7"
                style={{ '--i': '49' }}
              />{' '}
              <line
                className="t tide"
                x1="806.5"
                y1="174.8"
                x2="833.4"
                y2="153.9"
                style={{ '--i': '50' }}
              />{' '}
              <line
                className="t volt"
                x1="814.2"
                y1="185.1"
                x2="842"
                y2="165.6"
                style={{ '--i': '51' }}
              />{' '}
              <line
                className="t iris"
                x1="821.4"
                y1="195.8"
                x2="850.1"
                y2="177.6"
                style={{ '--i': '52' }}
              />{' '}
              <line
                className="t tide"
                x1="828"
                y1="206.9"
                x2="857.5"
                y2="190.1"
                style={{ '--i': '53' }}
              />{' '}
              <line
                className="t iris"
                x1="834"
                y1="218.2"
                x2="864.4"
                y2="202.9"
                style={{ '--i': '54' }}
              />{' '}
              <line
                className="t tide"
                x1="839.5"
                y1="229.8"
                x2="870.6"
                y2="216"
                style={{ '--i': '55' }}
              />{' '}
              <line
                className="t iris"
                x1="844.4"
                y1="241.7"
                x2="876.2"
                y2="229.5"
                style={{ '--i': '56' }}
              />{' '}
              <line
                className="t volt"
                x1="848.8"
                y1="253.8"
                x2="881.1"
                y2="243.2"
                style={{ '--i': '57' }}
              />{' '}
              <line
                className="t tide"
                x1="852.5"
                y1="266.1"
                x2="885.3"
                y2="257.1"
                style={{ '--i': '58' }}
              />{' '}
              <line
                className="t iris"
                x1="855.6"
                y1="278.6"
                x2="888.8"
                y2="271.1"
                style={{ '--i': '59' }}
              />{' '}
              <line
                className="t tide"
                x1="858.1"
                y1="291.2"
                x2="891.6"
                y2="285.4"
                style={{ '--i': '60' }}
              />{' '}
              <line
                className="t iris"
                x1="860"
                y1="303.9"
                x2="893.8"
                y2="299.8"
                style={{ '--i': '61' }}
              />{' '}
              <line
                className="t tide"
                x1="861.3"
                y1="316.7"
                x2="895.2"
                y2="314.2"
                style={{ '--i': '62' }}
              />{' '}
              <line
                className="t volt"
                x1="861.9"
                y1="329.6"
                x2="895.9"
                y2="328.7"
                style={{ '--i': '63' }}
              />{' '}
            </g>{' '}
          </svg>{' '}
        </div>{' '}
        <div className="wrap">
          {' '}
          <div className="foot-body">
            {' '}
            <div className="foot-brand">
              {' '}
              <Link className="wordmark" href="/" translate="no" aria-label="Fuselane home">
                {' '}
                <svg className="mark" viewBox="0 0 512 512" aria-hidden="true">
                  {' '}
                  <g fill="none" strokeLinecap="round" strokeWidth="34">
                    {' '}
                    <path
                      pathLength="1"
                      d="M70 150 C 180 150, 220 256, 300 256"
                      stroke="var(--lane-tide)"
                    />{' '}
                    <path pathLength="1" d="M70 256 L 300 256" stroke="var(--lane-volt)" />{' '}
                    <path
                      pathLength="1"
                      d="M70 362 C 180 362, 220 256, 300 256"
                      stroke="var(--lane-iris)"
                    />{' '}
                    <path
                      pathLength="1"
                      d="M300 256 L 442 256"
                      stroke="var(--fuse)"
                      strokeWidth="46"
                    />{' '}
                  </g>{' '}
                </svg>{' '}
                <span>Fuselane</span>{' '}
              </Link>{' '}
              <p>
                {' '}
                One download over every network you have. Free, open source, and only on your
                computer.{' '}
              </p>{' '}
            </div>{' '}
            <div className="foot-cols">
              {' '}
              <nav aria-labelledby="foot-get">
                {' '}
                <h2 id="foot-get">Get it</h2>{' '}
                <ul>
                  {' '}
                  <li>
                    <Link href="/download/">Download</Link>
                  </li>{' '}
                  <li>
                    <a href="https://github.com/ArshPunisher/fuselane/releases">Release notes</a>
                  </li>{' '}
                  <li>
                    <Link href="/guide/">Guide</Link>
                  </li>{' '}
                  <li>
                    <Link href="/faq/">FAQ</Link>
                  </li>{' '}
                </ul>{' '}
              </nav>{' '}
              <nav aria-labelledby="foot-project">
                {' '}
                <h2 id="foot-project">Project</h2>{' '}
                <ul>
                  {' '}
                  <li>
                    <a href="https://github.com/ArshPunisher/fuselane">Source code</a>
                  </li>{' '}
                  <li>
                    <Link href="/support/">Support</Link>
                  </li>{' '}
                  <li>
                    <Link href="/idm-alternative/">IDM alternative</Link>
                  </li>{' '}
                  <li>
                    <Link href="/combine-internet/">Combine connections</Link>
                  </li>{' '}
                  <li>
                    <Link href="/send-large-files/">Send large files</Link>
                  </li>{' '}
                  <li>
                    <a href="https://github.com/ArshPunisher/fuselane/issues">Report a problem</a>
                  </li>{' '}
                </ul>{' '}
              </nav>{' '}
              <nav aria-labelledby="foot-trust">
                {' '}
                <h2 id="foot-trust">Trust</h2>{' '}
                <ul>
                  {' '}
                  <li>
                    <Link href="/privacy/">Privacy</Link>
                  </li>{' '}
                  <li>
                    <Link href="/terms/">Terms</Link>
                  </li>{' '}
                  <li>
                    {' '}
                    <a href="https://github.com/ArshPunisher/fuselane/blob/main/LICENSE">
                      Apache-2.0 licence
                    </a>{' '}
                  </li>{' '}
                </ul>{' '}
              </nav>{' '}
            </div>{' '}
          </div>{' '}
          <div className="foot-base">
            {' '}
            <span>
              Free software under Apache-2.0. Made by{' '}
              <a href="https://github.com/ArshPunisher">Arsh Ramgarhia</a>.
            </span>{' '}
            <span className="foot-end">
              {' '}
              No account, no ads, no tracking.{' '}
              <button
                className="still-toggle"
                type="button"
                data-still-toggle=""
                aria-pressed="false"
              >
                {' '}
                Pause animations{' '}
              </button>{' '}
            </span>{' '}
          </div>{' '}
        </div>{' '}
      </footer>{' '}
    </>
  )
}
